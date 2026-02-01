//! Temporary directory management

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::Result;

/// Counter for unique directory names
static TEMP_COUNTER: AtomicUsize = AtomicUsize::new(0);

/// Create a unique temporary directory
pub fn create_temp_dir(prefix: &str) -> Result<PathBuf> {
    let base_dir = std::env::temp_dir();
    let unique = unique_suffix();
    let dir_name = format!(".{}-{}", prefix, unique);
    let temp_dir = base_dir.join(dir_name);
    fs::create_dir_all(&temp_dir)?;
    Ok(temp_dir)
}

/// Clean up a temporary directory
pub fn cleanup_temp_dir(temp_dir: &std::path::Path) {
    let _ = fs::remove_dir_all(temp_dir);
}

/// Generate a unique suffix using timestamp and counter
fn unique_suffix() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{}-{}", millis, counter)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_and_cleanup_temp_dir() {
        let dir1 = create_temp_dir("test").expect("create dir1");
        let dir2 = create_temp_dir("test").expect("create dir2");

        // Directories should be unique
        assert_ne!(dir1, dir2);
        assert!(dir1.exists());
        assert!(dir2.exists());

        // Cleanup
        cleanup_temp_dir(&dir1);
        cleanup_temp_dir(&dir2);

        assert!(!dir1.exists());
        assert!(!dir2.exists());
    }

    #[test]
    fn test_unique_suffix() {
        let s1 = unique_suffix();
        let s2 = unique_suffix();
        assert_ne!(s1, s2);
    }
}
