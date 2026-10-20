//! Read the files currently selected in File Explorer / Finder.
//!
//! Two steps, because reading the selection can be slow (Finder cold start: seconds):
//! 1. `source_is_frontmost()` — cheap; call on the hotkey *before* the popup takes focus.
//! 2. `get_selected_files()` — slow; call off the main thread after the popup is visible.

use std::path::PathBuf;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "macos")]
pub use macos::{get_selected_files, source_is_frontmost};
#[cfg(target_os = "windows")]
pub use windows::{get_selected_files, source_is_frontmost};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Selection {
    Files(Vec<PathBuf>),
    /// Nothing selected, or the selection couldn't be read.
    Empty,
    /// The OS refused access (macOS Automation permission). UI shows a hint.
    PermissionDenied,
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub fn source_is_frontmost() -> bool {
    false
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub fn get_selected_files() -> Selection {
    Selection::Empty
}

/// Parse newline-separated paths, ignoring blank lines.
pub(crate) fn parse_paths(output: &str) -> Vec<PathBuf> {
    output
        .lines()
        .map(str::trim_end)
        .filter(|line| !line.is_empty())
        .map(PathBuf::from)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_newline_separated_paths() {
        let out = "/Users/a/Photos/IMG 1.jpg\n/Users/a/doc.pdf\n\n";
        assert_eq!(
            parse_paths(out),
            vec![
                PathBuf::from("/Users/a/Photos/IMG 1.jpg"),
                PathBuf::from("/Users/a/doc.pdf")
            ]
        );
    }

    #[test]
    fn empty_output_is_no_files() {
        assert!(parse_paths("\n").is_empty());
    }
}
