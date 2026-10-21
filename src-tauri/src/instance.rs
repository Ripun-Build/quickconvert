//! One QuickConvert at a time: two copies would fight over the global shortcut, and the hotkey
//! would then seem to do nothing.

use std::fs::{File, OpenOptions, TryLockError};
use std::path::Path;

/// Held for the app's lifetime. The OS releases the lock when the process exits, even on a crash.
pub struct InstanceLock {
    _file: File,
}

/// Take the single-instance lock in `dir` (the app config folder, shared by every copy).
/// `Ok(None)`: another copy is already running.
pub fn acquire(dir: &Path) -> std::io::Result<Option<InstanceLock>> {
    std::fs::create_dir_all(dir)?;
    let file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(dir.join("instance.lock"))?;
    match file.try_lock() {
        Ok(()) => Ok(Some(InstanceLock { _file: file })),
        Err(TryLockError::WouldBlock) => Ok(None),
        Err(TryLockError::Error(err)) => Err(err),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn second_copy_is_refused_until_the_first_exits() {
        let dir = std::env::temp_dir().join(format!("qc-instance-{}", std::process::id()));
        let first = acquire(&dir).unwrap();
        assert!(first.is_some());
        assert!(
            acquire(&dir).unwrap().is_none(),
            "second copy must be refused"
        );
        drop(first);
        assert!(
            acquire(&dir).unwrap().is_some(),
            "free again once the first copy is gone"
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
}
