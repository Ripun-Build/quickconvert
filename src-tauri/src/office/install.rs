//! Install LibreOffice from inside QuickConvert, only when the user presses Install and confirms:
//! download the pinned version for this Mac's chip from LibreOffice's official server, check it
//! against the published SHA-256, then copy the app into Applications (or ~/Applications when
//! Applications isn't writable). This is QuickConvert's only use of the internet (SPECS §13).
//! It uses the Mac's own `curl`, `shasum`, `hdiutil` and `ditto`.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use serde::Serialize;

use crate::errors::AppError;
use crate::temp::TempFile;

/// The LibreOffice version QuickConvert installs (moved to the newest in each QuickConvert update).
pub const VERSION: &str = "26.8.1";

/// One download: where it is and what it must be.
#[derive(Debug, Clone)]
pub struct Package {
    pub urls: Vec<String>,
    pub bytes: u64,
    pub sha256: String,
}

/// The pinned package for this Mac's chip (each half of the universal app has its own).
pub fn package() -> Package {
    let (dir, file, bytes, sha256) = if cfg!(target_arch = "aarch64") {
        (
            "aarch64",
            "LibreOffice_26.8.1_MacOS_aarch64.dmg",
            298_845_477,
            "ca074e0b13571efb30a31fc531c5f95e997030637325e5d5de92d67aefdf0dfc",
        )
    } else {
        (
            "x86_64",
            "LibreOffice_26.8.1_MacOS_x86-64.dmg",
            309_092_706,
            "4c7464313a529e9074400cbcdc894fc40ac4ef9421ded03b16b5a78d12cb155e",
        )
    };
    Package {
        // The current release folder, then the permanent archive once a newer version replaces it.
        urls: vec![
            format!("https://download.documentfoundation.org/libreoffice/stable/{VERSION}/mac/{dir}/{file}"),
            format!("https://downloadarchive.documentfoundation.org/libreoffice/old/{VERSION}/mac/{dir}/{file}"),
        ],
        bytes,
        sha256: sha256.into(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Phase {
    Downloading,
    Checking,
    Installing,
    Done,
    Failed,
    Cancelled,
}

/// Payload of `office://install`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub phase: Phase,
    pub received: u64,
    pub total: u64,
    /// Plain-language reason when it failed.
    pub message: Option<String>,
    /// Where it was installed, when done.
    pub path: Option<String>,
}

impl Progress {
    pub fn at(phase: Phase, received: u64, total: u64) -> Self {
        Self {
            phase,
            received,
            total,
            message: None,
            path: None,
        }
    }
}

/// App state for the one install that may run at a time (the UI can close and reopen meanwhile).
#[derive(Default)]
pub struct InstallState {
    pub running: AtomicBool,
    pub cancel: AtomicBool,
    pub last: std::sync::Mutex<Option<Progress>>,
}

/// Download, check and install `package` into the first writable of `apps_dirs`. Reports progress
/// as it goes; stops (deleting the download) when `cancel` is set.
pub fn install(
    package: &Package,
    apps_dirs: &[PathBuf],
    cancel: &AtomicBool,
    report: &dyn Fn(Progress),
) -> Result<PathBuf, AppError> {
    if apps_dirs.iter().any(|d| d.join("LibreOffice.app").exists()) {
        return Err(AppError::invalid("LibreOffice is already installed."));
    }
    let dmg = TempFile::new("libreoffice", "dmg");
    let mut last_error = None;
    let mut downloaded = false;
    for url in &package.urls {
        match download(url, dmg.path(), package.bytes, cancel, report) {
            Ok(()) => {
                downloaded = true;
                break;
            }
            Err(AppError::Cancelled) => return Err(AppError::Cancelled),
            Err(err) => last_error = Some(err),
        }
    }
    if !downloaded {
        return Err(last_error.unwrap_or_else(|| AppError::internal("no download address")));
    }

    report(Progress::at(Phase::Checking, package.bytes, package.bytes));
    if sha256_of(dmg.path())? != package.sha256 {
        return Err(AppError::invalid(
            "The download didn't match LibreOffice's official file, so it wasn't installed. Please try again.",
        ));
    }
    if cancel.load(Ordering::Relaxed) {
        return Err(AppError::Cancelled);
    }
    report(Progress::at(
        Phase::Installing,
        package.bytes,
        package.bytes,
    ));
    install_from_dmg(dmg.path(), apps_dirs)
}

/// `curl` the file to `dest`, reporting its growing size; tried again from the next address if
/// this one fails.
fn download(
    url: &str,
    dest: &Path,
    total: u64,
    cancel: &AtomicBool,
    report: &dyn Fn(Progress),
) -> Result<(), AppError> {
    let _ = std::fs::remove_file(dest);
    let mut child = Command::new("/usr/bin/curl")
        .args([
            "--location",
            "--fail",
            "--silent",
            "--show-error",
            "--retry",
            "2",
        ])
        .args(["--connect-timeout", "20", "--output"])
        .arg(dest)
        .arg(url)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()?;
    loop {
        if cancel.load(Ordering::Relaxed) {
            let _ = child.kill();
            let _ = child.wait();
            let _ = std::fs::remove_file(dest);
            return Err(AppError::Cancelled);
        }
        let received = std::fs::metadata(dest).map(|m| m.len()).unwrap_or(0);
        report(Progress::at(Phase::Downloading, received.min(total), total));
        if let Some(status) = child.try_wait()? {
            if status.success() {
                return Ok(());
            }
            let mut detail = String::new();
            if let Some(mut err) = child.stderr.take() {
                use std::io::Read;
                let _ = err.read_to_string(&mut detail);
            }
            return Err(AppError::invalid(format!(
                "Couldn't download LibreOffice. Check the internet connection and try again. ({})",
                detail.trim()
            )));
        }
        std::thread::sleep(Duration::from_millis(250));
    }
}

/// SHA-256 of a file, as lower-case hex.
pub fn sha256_of(path: &Path) -> Result<String, AppError> {
    let out = Command::new("/usr/bin/shasum")
        .args(["-a", "256"])
        .arg(path)
        .output()?;
    let text = String::from_utf8_lossy(&out.stdout);
    text.split_whitespace()
        .next()
        .filter(|h| out.status.success() && h.len() == 64)
        .map(str::to_ascii_lowercase)
        .ok_or_else(|| AppError::internal("shasum failed"))
}

/// The first of `dirs` QuickConvert may write into (creating ~/Applications when needed).
pub fn writable_dir(dirs: &[PathBuf]) -> Option<PathBuf> {
    dirs.iter().find_map(|dir| {
        let _ = std::fs::create_dir_all(dir);
        let probe = dir.join(format!(".quickconvert-write-test-{}", std::process::id()));
        let ok = std::fs::write(&probe, b"").is_ok();
        let _ = std::fs::remove_file(&probe);
        ok.then(|| dir.clone())
    })
}

/// Open the disk image invisibly, copy LibreOffice.app out, and close it again.
pub fn install_from_dmg(dmg: &Path, apps_dirs: &[PathBuf]) -> Result<PathBuf, AppError> {
    let target_dir = writable_dir(apps_dirs).ok_or_else(|| {
        AppError::invalid("QuickConvert can't write to Applications. Install LibreOffice from libreoffice.org instead.")
    })?;
    let mount = std::env::temp_dir().join(format!("quickconvert-mount-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&mount);
    let attached = Command::new("/usr/bin/hdiutil")
        .args([
            "attach",
            "-nobrowse",
            "-readonly",
            "-noautoopen",
            "-noverify",
            "-mountpoint",
        ])
        .arg(&mount)
        .arg(dmg)
        .output()?;
    if !attached.status.success() {
        let _ = std::fs::remove_dir(&mount);
        return Err(AppError::Corrupt {
            detail: String::from_utf8_lossy(&attached.stderr).into_owned(),
        });
    }
    let result = (|| {
        let app = std::fs::read_dir(&mount)?
            .filter_map(Result::ok)
            .map(|e| e.path())
            .find(|p| p.file_name().is_some_and(|n| n == "LibreOffice.app"))
            .ok_or_else(|| AppError::Corrupt {
                detail: "no LibreOffice.app in the disk image".into(),
            })?;
        let target = target_dir.join("LibreOffice.app");
        let copied = Command::new("/usr/bin/ditto")
            .arg(&app)
            .arg(&target)
            .status()?;
        if !copied.success() {
            let _ = std::fs::remove_dir_all(&target);
            return Err(AppError::invalid(
                "Couldn't copy LibreOffice into Applications.",
            ));
        }
        Ok(target)
    })();
    let _ = Command::new("/usr/bin/hdiutil")
        .args(["detach", "-force"])
        .arg(&mount)
        .output();
    let _ = std::fs::remove_dir(&mount);
    result
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;
    use std::sync::Mutex;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("qc-install-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// A small disk image holding a fake LibreOffice.app.
    fn fake_dmg(dir: &Path) -> PathBuf {
        let src = dir.join("src");
        std::fs::create_dir_all(src.join("LibreOffice.app/Contents")).unwrap();
        std::fs::write(src.join("LibreOffice.app/Contents/Info.plist"), "<plist/>").unwrap();
        let dmg = dir.join("fake.dmg");
        let ok = Command::new("/usr/bin/hdiutil")
            .args([
                "create",
                "-quiet",
                "-fs",
                "HFS+",
                "-volname",
                "LibreOffice",
                "-srcfolder",
            ])
            .arg(&src)
            .arg(&dmg)
            .status()
            .unwrap()
            .success();
        assert!(ok);
        dmg
    }

    #[test]
    fn pinned_package_points_at_the_official_server() {
        let p = package();
        assert!(p.urls.iter().all(|u| u.starts_with("https://")
            && u.contains(".documentfoundation.org/")
            && u.contains(VERSION)));
        assert_eq!(p.sha256.len(), 64);
        assert!(p.bytes > 250_000_000);
    }

    #[test]
    fn whole_install_from_a_local_file_with_progress() {
        let dir = temp_dir("full");
        let dmg = fake_dmg(&dir);
        let apps = dir.join("Applications");
        let package = Package {
            urls: vec![
                format!("file://{}/missing.dmg", dir.display()), // first address fails
                format!("file://{}", dmg.display()),
            ],
            bytes: std::fs::metadata(&dmg).unwrap().len(),
            sha256: sha256_of(&dmg).unwrap(),
        };
        let seen = Mutex::new(Vec::new());
        let path = install(
            &package,
            std::slice::from_ref(&apps),
            &AtomicBool::new(false),
            &|p| seen.lock().unwrap().push(p.phase),
        )
        .unwrap();
        assert_eq!(path, apps.join("LibreOffice.app"));
        assert!(path.join("Contents/Info.plist").exists());
        let phases = seen.into_inner().unwrap();
        assert!(phases.contains(&Phase::Downloading) && phases.contains(&Phase::Checking));
        assert_eq!(phases.last(), Some(&Phase::Installing));

        let again = install(&package, &[apps], &AtomicBool::new(false), &|_| {}).unwrap_err();
        assert_eq!(again.to_string(), "LibreOffice is already installed.");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_download_that_doesnt_match_is_never_installed() {
        let dir = temp_dir("mismatch");
        let dmg = fake_dmg(&dir);
        let apps = dir.join("Applications");
        let package = Package {
            urls: vec![format!("file://{}", dmg.display())],
            bytes: 1,
            sha256: "0".repeat(64),
        };
        let err = install(
            &package,
            std::slice::from_ref(&apps),
            &AtomicBool::new(false),
            &|_| {},
        )
        .unwrap_err();
        assert!(err.to_string().starts_with("The download didn't match"));
        assert!(!apps.join("LibreOffice.app").exists());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn cancel_stops_before_anything_is_installed() {
        let dir = temp_dir("cancel");
        let dmg = fake_dmg(&dir);
        let apps = dir.join("Applications");
        let package = Package {
            urls: vec![format!("file://{}", dmg.display())],
            bytes: 1,
            sha256: sha256_of(&dmg).unwrap(),
        };
        let err = install(
            &package,
            std::slice::from_ref(&apps),
            &AtomicBool::new(true),
            &|_| {},
        )
        .unwrap_err();
        assert!(matches!(err, AppError::Cancelled));
        assert!(!apps.join("LibreOffice.app").exists());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn falls_back_to_the_users_own_applications_folder() {
        use std::os::unix::fs::PermissionsExt;
        let dir = temp_dir("fallback");
        let locked = dir.join("Applications");
        std::fs::create_dir_all(&locked).unwrap();
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o555)).unwrap();
        let mine = dir.join("home/Applications");
        assert_eq!(writable_dir(&[locked.clone(), mine.clone()]), Some(mine));
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).unwrap();
        std::fs::remove_dir_all(dir).unwrap();
    }
}
