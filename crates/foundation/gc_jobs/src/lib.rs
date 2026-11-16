//! Background jobs with cancellation, progress and panic isolation.
//!
//! Long work (path planning, collision-matrix generation, mesh import) runs as a job so the user
//! interface never blocks. Where a job runs is decided by the [`Executor`] that is injected:
//!
//! - [`Executor::thread`]: a new thread per job (the application).
//! - [`Executor::inline`]: immediately on the calling thread (deterministic headless tests).
//! - [`Executor::manual`]: queued until the test runs it (tests of "job still running" states).
//!
//! A job that panics does not take the application down: its handle reports [`JobOutcome::Failed`].

#![forbid(unsafe_code)]
#![cfg_attr(test, allow(clippy::float_cmp, reason = "tests compare exactly representable values"))]

use std::any::Any;
use std::collections::VecDeque;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex, PoisonError};

/// Identifies a job within the process.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct JobId(pub u64);

/// A flag the owner of a job sets to ask the job to stop. Jobs poll it at safe points.
#[derive(Clone, Debug, Default)]
pub struct CancelToken(Arc<AtomicBool>);

impl CancelToken {
    /// Asks the job to stop.
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Relaxed);
    }

    /// True once [`CancelToken::cancel`] was called.
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

/// How far a job has come, from 0 to 1.
#[derive(Clone, Debug, Default)]
pub struct Progress(Arc<AtomicU32>);

impl Progress {
    /// Sets the completed fraction; values outside `0..=1` are clamped and NaN counts as 0.
    pub fn set(&self, fraction: f32) {
        let clamped = if fraction.is_nan() { 0.0 } else { fraction.clamp(0.0, 1.0) };
        self.0.store(clamped.to_bits(), Ordering::Relaxed);
    }

    /// The completed fraction.
    pub fn get(&self) -> f32 {
        f32::from_bits(self.0.load(Ordering::Relaxed))
    }
}

/// What a running job can see of the outside world.
#[derive(Clone, Debug, Default)]
pub struct JobCtx {
    /// Poll this and return early when it is set.
    pub cancel: CancelToken,
    /// Report progress here.
    pub progress: Progress,
}

/// Called from the job's thread when a job finishes, e.g. to request a repaint of the user interface.
pub trait Notifier: Send + Sync {
    /// A job has finished and its result can be collected.
    fn notify(&self);
}

/// How a job ended.
#[derive(Debug, PartialEq)]
pub enum JobOutcome<T> {
    /// The work function returned this value. Cancellation is part of `T` when the job supports it.
    Done(T),
    /// The job panicked or could not be started; the text is for the log.
    Failed(String),
}

/// The owner's side of a job.
#[derive(Debug)]
pub struct JobHandle<T> {
    id: JobId,
    ctx: JobCtx,
    result: Receiver<JobOutcome<T>>,
}

impl<T> JobHandle<T> {
    /// The job's id.
    pub fn id(&self) -> JobId {
        self.id
    }

    /// Asks the job to stop. The outcome still arrives through [`JobHandle::poll`].
    pub fn cancel(&self) {
        self.ctx.cancel.cancel();
    }

    /// The completed fraction reported by the job.
    pub fn progress(&self) -> f32 {
        self.ctx.progress.get()
    }

    /// Takes the outcome if the job has finished; `None` while it is running. After the outcome was taken,
    /// further calls return `None`.
    pub fn poll(&mut self) -> Option<JobOutcome<T>> {
        self.result.try_recv().ok()
    }
}

type Task = Box<dyn FnOnce() + Send>;

/// The queue behind [`Executor::manual`]; tests keep a clone and decide when jobs run.
#[derive(Clone, Default)]
pub struct ManualQueue(Arc<Mutex<VecDeque<Task>>>);

impl ManualQueue {
    /// Runs the oldest queued job to completion. Returns `false` when the queue is empty.
    pub fn run_next(&self) -> bool {
        let task = self.0.lock().unwrap_or_else(PoisonError::into_inner).pop_front();
        task.map(|task| task()).is_some()
    }

    /// Runs queued jobs until none is left and returns how many ran.
    pub fn run_all(&self) -> usize {
        let mut count = 0;
        while self.run_next() {
            count += 1;
        }
        count
    }

    /// Number of jobs waiting.
    pub fn len(&self) -> usize {
        self.0.lock().unwrap_or_else(PoisonError::into_inner).len()
    }

    /// True when no job is waiting.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[derive(Clone)]
enum Mode {
    Thread,
    Inline,
    Manual(ManualQueue),
}

/// Decides where jobs run. Cheap to clone.
#[derive(Clone)]
pub struct Executor {
    mode: Mode,
    notifier: Option<Arc<dyn Notifier>>,
}

static NEXT_JOB_ID: AtomicU64 = AtomicU64::new(1);

fn panic_text(payload: &(dyn Any + Send)) -> String {
    if let Some(text) = payload.downcast_ref::<&str>() {
        (*text).to_owned()
    } else if let Some(text) = payload.downcast_ref::<String>() {
        text.clone()
    } else {
        "job panicked".to_owned()
    }
}

impl Executor {
    /// Runs each job on its own thread.
    pub fn thread() -> Self {
        Self { mode: Mode::Thread, notifier: None }
    }

    /// Runs each job to completion inside `spawn`.
    pub fn inline() -> Self {
        Self { mode: Mode::Inline, notifier: None }
    }

    /// Queues jobs until the returned queue is told to run them.
    pub fn manual() -> (Self, ManualQueue) {
        let queue = ManualQueue::default();
        (Self { mode: Mode::Manual(queue.clone()), notifier: None }, queue)
    }

    /// Calls `notifier` whenever a job finishes.
    #[must_use]
    pub fn with_notifier(mut self, notifier: Arc<dyn Notifier>) -> Self {
        self.notifier = Some(notifier);
        self
    }

    /// Starts a job. `name` labels the thread and the log entry.
    pub fn spawn<T, F>(&self, name: &str, work: F) -> JobHandle<T>
    where
        T: Send + 'static,
        F: FnOnce(&JobCtx) -> T + Send + 'static,
    {
        let id = JobId(NEXT_JOB_ID.fetch_add(1, Ordering::Relaxed));
        let ctx = JobCtx::default();
        let (sender, result) = channel();
        let job_ctx = ctx.clone();
        let notifier = self.notifier.clone();
        let failure_sender = sender.clone();
        let task = move || {
            // The job owns everything it touches, so observing state after a panic is not a concern.
            let outcome = match catch_unwind(AssertUnwindSafe(|| work(&job_ctx))) {
                Ok(value) => JobOutcome::Done(value),
                Err(payload) => JobOutcome::Failed(panic_text(payload.as_ref())),
            };
            // The handle may be gone already; then nobody wants the result.
            drop(sender.send(outcome));
            if let Some(notifier) = notifier {
                notifier.notify();
            }
        };
        match &self.mode {
            Mode::Inline => task(),
            Mode::Manual(queue) => {
                queue.0.lock().unwrap_or_else(PoisonError::into_inner).push_back(Box::new(task))
            }
            Mode::Thread => {
                let spawned = std::thread::Builder::new().name(format!("gc-job:{name}")).spawn(task);
                if let Err(error) = spawned {
                    drop(failure_sender.send(JobOutcome::Failed(format!("could not start thread: {error}"))));
                }
            }
        }
        JobHandle { id, ctx, result }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicUsize;
    use std::time::{Duration, Instant};

    use super::*;

    fn wait<T>(handle: &mut JobHandle<T>) -> JobOutcome<T> {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if let Some(outcome) = handle.poll() {
                return outcome;
            }
            assert!(Instant::now() < deadline, "job did not finish");
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    #[test]
    fn inline_jobs_finish_inside_spawn() {
        let mut handle = Executor::inline().spawn("sum", |ctx| {
            ctx.progress.set(1.0);
            2 + 2
        });
        assert_eq!(handle.progress(), 1.0);
        assert_eq!(handle.poll(), Some(JobOutcome::Done(4)));
        assert_eq!(handle.poll(), None, "the outcome is delivered once");
    }

    #[test]
    fn thread_jobs_deliver_their_result() {
        let mut handle = Executor::thread().spawn("answer", |_| 42);
        assert_eq!(wait(&mut handle), JobOutcome::Done(42));
    }

    #[test]
    fn manual_jobs_run_only_when_told_and_in_order() {
        let (executor, queue) = Executor::manual();
        let mut first = executor.spawn("first", |_| 1);
        let mut second = executor.spawn("second", |_| 2);
        assert_eq!((first.poll(), queue.len()), (None, 2));
        assert!(queue.run_next());
        assert_eq!(first.poll(), Some(JobOutcome::Done(1)));
        assert_eq!(second.poll(), None);
        assert_eq!(queue.run_all(), 1);
        assert_eq!(second.poll(), Some(JobOutcome::Done(2)));
        assert!(!queue.run_next());
        assert!(queue.is_empty());
    }

    #[test]
    fn a_panicking_job_reports_failure_instead_of_crashing() {
        let mut handle = Executor::inline().spawn("boom", |_| -> u32 { panic!("planner exploded") });
        assert_eq!(handle.poll(), Some(JobOutcome::Failed("planner exploded".to_owned())));
        let mut threaded =
            Executor::thread().spawn("boom", |_| -> u32 { panic!("{}", String::from("formatted")) });
        assert_eq!(wait(&mut threaded), JobOutcome::Failed("formatted".to_owned()));
    }

    #[test]
    fn cancellation_reaches_a_running_job_quickly() {
        let mut handle = Executor::thread().spawn("spin", |ctx| {
            let mut polls = 0_u64;
            while !ctx.cancel.is_cancelled() {
                polls += 1;
                std::thread::yield_now();
            }
            polls
        });
        std::thread::sleep(Duration::from_millis(5));
        let asked = Instant::now();
        handle.cancel();
        assert!(matches!(wait(&mut handle), JobOutcome::Done(_)));
        assert!(asked.elapsed() < Duration::from_millis(100), "took {:?}", asked.elapsed());
    }

    #[test]
    fn the_notifier_fires_once_per_finished_job() {
        struct Counter(AtomicUsize);
        impl Notifier for Counter {
            fn notify(&self) {
                self.0.fetch_add(1, Ordering::Relaxed);
            }
        }
        let counter = Arc::new(Counter(AtomicUsize::new(0)));
        let executor = Executor::inline().with_notifier(counter.clone());
        drop(executor.spawn("a", |_| ()));
        drop(executor.spawn("b", |_| -> () { panic!("x") }));
        assert_eq!(counter.0.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn progress_is_clamped() {
        let progress = Progress::default();
        assert_eq!(progress.get(), 0.0);
        progress.set(0.25);
        assert_eq!(progress.get(), 0.25);
        progress.set(7.0);
        assert_eq!(progress.get(), 1.0);
        progress.set(f32::NAN);
        assert_eq!(progress.get(), 0.0);
    }

    #[test]
    fn job_ids_are_distinct() {
        let executor = Executor::inline();
        assert_ne!(executor.spawn("a", |_| ()).id(), executor.spawn("b", |_| ()).id());
    }
}
