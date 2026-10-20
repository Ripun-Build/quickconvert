//! Explorer selection via Shell COM on an STA thread. Stub until the Windows port (Phase 2):
//! the popup then opens in the drop zone.

use super::Selection;

pub fn source_is_frontmost() -> bool {
    false
}

pub fn get_selected_files() -> Selection {
    Selection::Empty
}
