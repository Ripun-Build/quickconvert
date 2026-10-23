//! Run independent pieces of work a few at a time (batch conversions, PDF pages), keeping results
//! in input order. Plain scoped threads, no extra dependency.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

/// Files worked on at once: enough to use the cores, few enough that several 12 MP photos in memory
/// at the same time stay well within RAM.
pub fn default_workers() -> usize {
    std::thread::available_parallelism().map_or(2, |n| n.get().clamp(1, 4))
}

/// `work(i)` for every i in 0..count on up to `workers` threads, results in input order. Each
/// thread takes the next index as it frees up, so work starts in input order. A slot is None only
/// if its worker panicked.
pub fn map<T: Send>(
    count: usize,
    workers: usize,
    work: impl Fn(usize) -> T + Sync,
) -> Vec<Option<T>> {
    let next = AtomicUsize::new(0);
    let slots: Vec<Mutex<Option<T>>> = (0..count).map(|_| Mutex::new(None)).collect();
    std::thread::scope(|scope| {
        for _ in 0..workers.clamp(1, count.max(1)) {
            scope.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::Relaxed);
                if i >= count {
                    break;
                }
                let result = work(i);
                if let Ok(mut slot) = slots[i].lock() {
                    *slot = Some(result);
                }
            });
        }
    });
    slots
        .into_iter()
        .map(|slot| slot.into_inner().ok().flatten())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::time::Duration;

    #[test]
    fn results_come_back_in_input_order() {
        let out = map(20, 4, |i| {
            std::thread::sleep(Duration::from_millis((20 - i as u64) % 7));
            i * 10
        });
        assert_eq!(out, (0..20).map(|i| Some(i * 10)).collect::<Vec<_>>());
    }

    #[test]
    fn work_is_shared_between_threads() {
        let threads = Mutex::new(HashSet::new());
        map(8, 4, |_| {
            threads.lock().unwrap().insert(std::thread::current().id());
            std::thread::sleep(Duration::from_millis(30));
        });
        assert!(threads.lock().unwrap().len() > 1);
    }

    #[test]
    fn one_worker_runs_in_order_and_nothing_is_empty() {
        let seen = Mutex::new(Vec::new());
        map(5, 1, |i| seen.lock().unwrap().push(i));
        assert_eq!(*seen.lock().unwrap(), [0, 1, 2, 3, 4]);
        assert!(map(0, 4, |i| i).is_empty());
    }
}
