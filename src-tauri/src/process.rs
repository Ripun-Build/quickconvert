//! Outside tools (osascript, sips, soffice, …) are started through `command`, so on Windows
//! they never flash a console window over the popup.

use std::ffi::OsStr;
use std::process::Command;

/// `Command::new(program)`, set up to run without a visible window.
pub fn command<S: AsRef<OsStr>>(program: S) -> Command {
    #[allow(unused_mut)]
    let mut cmd = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}
