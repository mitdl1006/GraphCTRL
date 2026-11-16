//! Evaluation order of parameters that refer to each other.

use std::collections::{BTreeMap, BTreeSet};

/// Sorts keys so that each comes after everything it depends on (Kahn's algorithm, deterministic).
///
/// `deps[k]` lists what `k` uses; entries that are not keys of `deps` are ignored. Returns the order and
/// the keys that could not be ordered because they are on a cycle or depend on one.
pub fn eval_order<K: Ord + Copy>(deps: &BTreeMap<K, BTreeSet<K>>) -> (Vec<K>, BTreeSet<K>) {
    let mut pending: BTreeMap<K, usize> =
        deps.iter().map(|(key, uses)| (*key, uses.iter().filter(|u| deps.contains_key(u)).count())).collect();
    let mut users: BTreeMap<K, Vec<K>> = BTreeMap::new();
    for (key, uses) in deps {
        for used in uses.iter().filter(|u| deps.contains_key(u)) {
            users.entry(*used).or_default().push(*key);
        }
    }
    let mut ready: BTreeSet<K> =
        pending.iter().filter(|(_, count)| **count == 0).map(|(key, _)| *key).collect();
    let mut order = Vec::with_capacity(deps.len());
    while let Some(key) = ready.pop_first() {
        order.push(key);
        for user in users.get(&key).into_iter().flatten() {
            if let Some(count) = pending.get_mut(user) {
                *count -= 1;
                if *count == 0 {
                    ready.insert(*user);
                }
            }
        }
    }
    let blocked = pending.into_iter().filter(|(_, count)| *count > 0).map(|(key, _)| key).collect();
    (order, blocked)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn graph(edges: &[(u32, &[u32])]) -> BTreeMap<u32, BTreeSet<u32>> {
        edges.iter().map(|(key, uses)| (*key, uses.iter().copied().collect())).collect()
    }

    #[test]
    fn dependencies_come_first_and_ties_follow_key_order() {
        let (order, blocked) = eval_order(&graph(&[(1, &[3]), (2, &[]), (3, &[2]), (4, &[])]));
        assert_eq!(order, vec![2, 3, 1, 4]);
        assert!(blocked.is_empty());
    }

    #[test]
    fn cycles_and_everything_behind_them_are_reported_not_looped_on() {
        let (order, blocked) = eval_order(&graph(&[(1, &[2]), (2, &[1]), (3, &[2]), (4, &[]), (5, &[5])]));
        assert_eq!(order, vec![4]);
        assert_eq!(blocked, [1, 2, 3, 5].into_iter().collect());
    }

    #[test]
    fn unknown_dependencies_are_ignored() {
        let (order, blocked) = eval_order(&graph(&[(1, &[99])]));
        assert_eq!(order, vec![1]);
        assert!(blocked.is_empty());
    }
}
