//! Finder selection via AppleScript (`osascript`).

use std::process::Command;

use super::{parse_paths, Selection};

const FINDER_BUNDLE_ID: &str = "com.apple.finder";

/// AppleScript error "Not authorized to send Apple events" (Automation permission denied).
const NOT_AUTHORIZED: &str = "-1743";

const SCRIPT: &str = r#"
with timeout of 5 seconds
    tell application "Finder"
        set out to ""
        repeat with f in (get selection)
            set out to out & POSIX path of (f as alias) & linefeed
        end repeat
        return out
    end tell
end timeout
"#;

/// Whether Finder is the frontmost app (~15 ms, no permissions needed).
pub fn source_is_frontmost() -> bool {
    let Some(asn) = run("lsappinfo", &["front"]) else {
        return false;
    };
    run("lsappinfo", &["info", "-only", "bundleid", asn.trim()])
        .is_some_and(|info| info.contains(&format!("\"{FINDER_BUNDLE_ID}\"")))
}

/// POSIX paths of the items selected in the frontmost Finder window (or the desktop).
pub fn get_selected_files() -> Selection {
    let output = match Command::new("osascript").args(["-e", SCRIPT]).output() {
        Ok(output) => output,
        Err(_) => return Selection::Empty,
    };
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return if stderr.contains(NOT_AUTHORIZED) {
            Selection::PermissionDenied
        } else {
            Selection::Empty
        };
    }
    let paths = parse_paths(&String::from_utf8_lossy(&output.stdout));
    if paths.is_empty() {
        Selection::Empty
    } else {
        Selection::Files(paths)
    }
}

fn run(program: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(program).args(args).output().ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).into_owned())
}
