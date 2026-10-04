//! Quitting and starting again, for a setting that only a new launch picks up.
//!
//! The new instance is spawned from the old one's last moment, once its event
//! loop has been torn down: every window has been dropped by then, so what
//! windows save on the way out (`state.json`, a preferences edit still
//! settling) is on disk. It is told the old one's process id and waits for
//! that process to be gone before it does anything else ([`wait_for_exit`]).
//! Until then the old instance still holds the IPC endpoint — on Windows a
//! named pipe that nothing removes early — and a new instance that found it
//! would hand its launch to a process that is exiting and leave nothing
//! running.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// The long flag (without its dashes) that carries the old instance's process id to the new one.
pub const AFTER_EXIT_OF: &str = "after-exit-of";

/// How long a new instance waits for the old one before starting anyway. An
/// exiting process is gone in well under a second; this only bounds a hang.
const EXIT_TIMEOUT: Duration = Duration::from_secs(10);

static REQUESTED: AtomicBool = AtomicBool::new(false);

/// Close every window and start Arto again once the app has exited.
pub(crate) fn request() {
    tracing::info!("relaunch requested");
    REQUESTED.store(true, Ordering::SeqCst);
    crate::window::shutdown_all_windows();
}

/// Start the new instance, if one was asked for. Called as the event loop is
/// destroyed, just before the process exits.
pub(crate) fn on_loop_destroyed() {
    if !REQUESTED.load(Ordering::SeqCst) {
        return;
    }
    crate::ipc::cleanup_socket();
    let exe = match std::env::current_exe() {
        Ok(exe) => exe,
        Err(error) => {
            tracing::error!(%error, "cannot relaunch: the executable is unknown");
            return;
        }
    };
    if let Err(error) = command(&exe, std::process::id()).spawn() {
        tracing::error!(%error, ?exe, "cannot relaunch");
    }
}

/// Hold a starting instance until the process it replaces has exited, so the
/// two never run side by side.
pub fn wait_for_exit(pid: u32) {
    if !wait_for_exit_within(pid, EXIT_TIMEOUT) {
        tracing::warn!(
            pid,
            "the instance being replaced is still running; starting anyway"
        );
    }
}

/// Whether `pid` was gone before `timeout` ran out.
fn wait_for_exit_within(pid: u32, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    loop {
        if !is_running(pid) {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[cfg(unix)]
fn is_running(pid: u32) -> bool {
    let Ok(pid) = libc::pid_t::try_from(pid) else {
        return false;
    };
    // Signal 0 checks that the process exists without touching it. EPERM
    // means it exists and belongs to someone else.
    if unsafe { libc::kill(pid, 0) } == 0 {
        return true;
    }
    std::io::Error::last_os_error().raw_os_error() != Some(libc::ESRCH)
}

#[cfg(windows)]
fn is_running(pid: u32) -> bool {
    use windows_sys::Win32::Foundation::{CloseHandle, WAIT_TIMEOUT};
    use windows_sys::Win32::System::Threading::{
        OpenProcess, WaitForSingleObject, PROCESS_SYNCHRONIZE,
    };

    let handle = unsafe { OpenProcess(PROCESS_SYNCHRONIZE, 0, pid) };
    if handle.is_null() {
        return false;
    }
    // A process handle is signalled once the process has exited.
    let running = unsafe { WaitForSingleObject(handle, 0) } == WAIT_TIMEOUT;
    unsafe { CloseHandle(handle) };
    running
}

/// What starts Arto again, told to wait for `pid`. An app bundle goes through
/// `open` so that macOS launches it as the app — Dock icon, menu bar and all —
/// rather than as a child of the process that is exiting.
fn command(exe: &Path, pid: u32) -> Command {
    let wait = [format!("--{AFTER_EXIT_OF}"), pid.to_string()];
    match bundle_of(exe) {
        Some(bundle) => {
            let mut command = Command::new("/usr/bin/open");
            // `-n`: the instance asking is still alive, and without it
            // LaunchServices would bring that one forward instead.
            command.arg("-n").arg(bundle).arg("--args").args(wait);
            command
        }
        None => {
            let mut command = Command::new(exe);
            command.args(wait);
            command
        }
    }
}

/// The `.app` directory `exe` runs from, when it is a bundle's executable.
fn bundle_of(exe: &Path) -> Option<PathBuf> {
    if !cfg!(target_os = "macos") {
        return None;
    }
    let macos = exe.parent()?;
    let contents = macos.parent()?;
    let bundle = contents.parent()?;
    let is_bundle = macos.file_name()? == "MacOS"
        && contents.file_name()? == "Contents"
        && bundle.extension()? == "app";
    is_bundle.then(|| bundle.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(target_os = "macos")]
    fn a_bundle_executable_is_relaunched_as_its_bundle() {
        let exe = Path::new("/Applications/Arto.app/Contents/MacOS/arto");
        assert_eq!(
            bundle_of(exe),
            Some(PathBuf::from("/Applications/Arto.app"))
        );
        let command = command(exe, 42);
        assert_eq!(command.get_program(), "/usr/bin/open");
        assert_eq!(
            command.get_args().collect::<Vec<_>>(),
            [
                "-n",
                "/Applications/Arto.app",
                "--args",
                "--after-exit-of",
                "42"
            ]
        );
    }

    #[test]
    fn a_loose_executable_is_run_again_as_it_is() {
        for exe in [
            "/home/user/.cargo/bin/arto",
            "/work/target/debug/arto",
            // Shaped like a bundle but not named like one.
            "/opt/Arto/Contents/MacOS/arto",
        ] {
            let exe = Path::new(exe);
            assert_eq!(bundle_of(exe), None, "{exe:?}");
            let command = command(exe, 42);
            assert_eq!(command.get_program(), exe);
            assert_eq!(
                command.get_args().collect::<Vec<_>>(),
                ["--after-exit-of", "42"]
            );
        }
    }

    #[test]
    fn waiting_ends_once_the_process_has_exited() {
        let mut child = Command::new(std::env::current_exe().unwrap())
            .arg("--list")
            .stdout(std::process::Stdio::null())
            .spawn()
            .unwrap();
        let pid = child.id();
        // Reaped, so the id no longer names a process (a zombie still would).
        child.wait().unwrap();
        assert!(wait_for_exit_within(pid, Duration::from_secs(5)));
    }

    #[test]
    fn waiting_gives_up_on_a_process_that_keeps_running() {
        assert!(!wait_for_exit_within(
            std::process::id(),
            Duration::from_millis(100)
        ));
    }
}
