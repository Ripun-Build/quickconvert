//! Read the files currently selected in File Explorer / Finder.
//!
//! Returns an empty list when nothing is selected or the selection can't be read;
//! the UI then shows the drop zone.

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "macos")]
pub use macos::get_selected_files;
#[cfg(target_os = "windows")]
pub use windows::get_selected_files;

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub fn get_selected_files() -> Vec<std::path::PathBuf> {
    Vec::new()
}
