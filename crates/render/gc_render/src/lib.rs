//! wgpu renderer drawing into an offscreen target; no egui dependency.
//!
//! The renderer only needs a `wgpu::Device` and `wgpu::Queue`. It draws into its own [`ViewTarget`]
//! (4x MSAA colour + reverse-Z depth, resolved to an `Rgba8Unorm` texture). A UI toolkit shows the
//! resolved texture; tests read it back with [`read_rgba`].

mod mesh;
mod target;

pub use mesh::{DrawStats, Instance, MeshRenderer, View};
pub use target::{RenderError, ViewTarget, clear, read_rgba};

/// Sample count used for the multisampled colour and depth attachments.
pub const MSAA_SAMPLES: u32 = 4;
/// Format of the resolved colour texture handed to the UI.
pub const COLOR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
/// Depth format (reverse-Z: cleared to 0, compared with `Greater`).
pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
