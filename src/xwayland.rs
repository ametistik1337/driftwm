//! xwayland-satellite integration.
//!
//! Spawns `xwayland-satellite` eagerly at compositor startup so X11 apps
//! can connect immediately.
//!
//! Background: the on-demand `-listenfd` pattern (compositor pre-binds the
//! X11 socket and hands the FD to satellite on first X11 connection) has an
//! interop bug with Xwayland 24.x and multi-layout XKB configs (e.g.
//! `layout = "us,ru"` + `options = "grp:win_space_toggle"`): the queued X11
//! connection on the pre-bound socket triggers Xwayland's keyboard
//! initialization before the `wl_keyboard.keymap` event arrives, causing
//! `XKB: Failed to compile keymap` and a satellite panic. Vanilla mode
//! (satellite binds its own X11 socket on startup) avoids the race entirely.
//! Trade-off: ~30MB resident even if no X11 client ever runs. Acceptable;
//! if upstream fixes the listenfd path we can revisit.

use std::os::unix::process::CommandExt;
use std::process::{Child, Command, Stdio};

use smithay::reexports::rustix;
use smithay::reexports::rustix::io::Errno;

use crate::state::DriftWm;

const MAX_DISPLAY: u32 = 50;

pub struct Satellite {
    /// Held for the satellite's lifetime; satellite exits when its Wayland
    /// connection closes on drop.
    pub child: Child,
    pub display_name: String,
}

impl Drop for Satellite {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Check if the satellite child process has exited, and respawn it if needed.
pub fn maintain_satellite(state: &mut DriftWm) {
    if !state.config.xwayland_enabled {
        return;
    }

    let is_dead = match &mut state.satellite {
        Some(s) => match s.child.try_wait() {
            Ok(Some(status)) => {
                tracing::warn!(
                    "xwayland-satellite on {} exited with status {status}, respawning...",
                    s.display_name
                );
                true
            }
            Ok(None) => false,
            Err(e) => {
                let pid = s.child.id() as libc::pid_t;
                let alive = unsafe {
                    libc::kill(pid, 0) == 0
                        || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
                };
                if !alive {
                    tracing::warn!(
                        "xwayland-satellite pid {pid} on {} is dead ({e}), respawning...",
                        s.display_name
                    );
                    true
                } else {
                    false
                }
            }
        },
        None => true,
    };

    if is_dead {
        state.satellite = None;
        setup(state);
    }
}

/// Spawn `xwayland-satellite :N` eagerly, export `DISPLAY=:N`. Fails soft:
/// any error leaves `state.satellite = None` so X11 apps don't work but
/// the compositor runs.
pub fn setup(state: &mut DriftWm) {
    if state.satellite.is_some() {
        return;
    }
    if !state.config.xwayland_enabled {
        return;
    }

    let path = state.config.xwayland_path.clone();
    if !probe_satellite(&path) {
        return;
    }

    let display = match find_free_display() {
        Some(n) => n,
        None => {
            tracing::warn!("no free X11 display number found, disabling xwayland-satellite");
            return;
        }
    };
    let display_name = format!(":{display}");

    let mut process = Command::new(&path);
    process
        .arg(&display_name)
        .envs(&state.config.child_env)
        .env_remove("DISPLAY")
        .env_remove("RUST_BACKTRACE")
        .env_remove("RUST_LIB_BACKTRACE")
        .stdin(Stdio::null())
        .stdout(Stdio::null());
    // stderr inherits so satellite startup messages surface alongside our log.

    unsafe {
        process.pre_exec(|| {
            // Don't pass the compositor's blocked sigmask to satellite.
            crate::signals::unblock_all()?;
            // SIGCHLD inherits as SIG_IGN from driftwm — satellite needs
            // SIG_DFL so it can wait() on its own children (Xwayland).
            libc::signal(libc::SIGCHLD, libc::SIG_DFL);
            Ok(())
        });
    }

    let child = match process.spawn() {
        Ok(c) => c,
        Err(err) => {
            tracing::warn!("failed to spawn xwayland-satellite ({path:?}): {err}");
            return;
        }
    };

    tracing::info!(
        "spawned xwayland-satellite pid={} on {display_name}",
        child.id()
    );

    // Make DISPLAY visible to children via child_env (process env untouched).
    state
        .config
        .child_env
        .insert("DISPLAY".into(), display_name.clone());

    export_display(&display_name);

    state.satellite = Some(Satellite {
        child,
        display_name,
    });
}

/// Probe whether the binary at `path` is launchable and supports our
/// expected protocol. Uses `--test-listenfd-support` because it's a cheap
/// "binary exists and responds to argv" check that all satellites since 0.7
/// implement and exit zero on. We don't actually use listenfd downstream.
fn probe_satellite(path: &str) -> bool {
    let mut process = Command::new(path);
    process
        .args([":0", "--test-listenfd-support"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .env_remove("DISPLAY")
        .env_remove("RUST_BACKTRACE")
        .env_remove("RUST_LIB_BACKTRACE");

    let mut child = match process.spawn() {
        Ok(c) => c,
        Err(err) => {
            tracing::warn!(
                "xwayland-satellite not found at {path:?}: {err} — X11 apps disabled \
                 (install xwayland-satellite, or set [xwayland] enabled = false to silence)"
            );
            return false;
        }
    };
    match child.wait() {
        Ok(s) if s.success() => true,
        Ok(_) => {
            tracing::warn!(
                "xwayland-satellite at {path:?} is too old (need >= 0.7) — X11 apps disabled"
            );
            false
        }
        Err(err) => {
            tracing::warn!("error waiting for xwayland-satellite probe: {err}");
            false
        }
    }
}

/// Display number with both lock file AND unix socket absent — either
/// artifact present means another X server (SDDM Xorg greeter, etc.) holds
/// it. Non-ENOENT stat errors count as occupied (so a root-owned lock file
/// with no read permission doesn't look free).
fn find_free_display() -> Option<u32> {
    (0..MAX_DISPLAY).find(|&n| !display_in_use(n))
}

fn display_in_use(n: u32) -> bool {
    let lock_path = format!("/tmp/.X{n}-lock");
    let sock_path = format!("/tmp/.X11-unix/X{n}");

    // Check lock file
    let lock_held = if path_present(&lock_path) {
        if let Ok(content) = std::fs::read_to_string(&lock_path) {
            if let Some(pid_str) = content.split_whitespace().next() {
                if let Ok(pid) = pid_str.parse::<i32>() {
                    let res = unsafe { libc::kill(pid, 0) };
                    if res == -1 && std::io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH) {
                        tracing::info!("Removing stale X11 lock file: {lock_path} (pid {pid} is dead)");
                        let _ = std::fs::remove_file(&lock_path);
                        false
                    } else {
                        true
                    }
                } else {
                    true
                }
            } else {
                true
            }
        } else {
            true
        }
    } else {
        false
    };

    if lock_held {
        return true;
    }

    // Check unix domain socket
    if path_present(&sock_path) {
        use std::os::unix::net::UnixStream;
        match UnixStream::connect(&sock_path) {
            Ok(_) => {
                // Socket is actively served
                true
            }
            Err(e) if e.kind() == std::io::ErrorKind::ConnectionRefused => {
                tracing::info!("Removing stale X11 socket: {sock_path}");
                let _ = std::fs::remove_file(&sock_path);
                false
            }
            Err(_) => {
                // Permission denied or other error -> assume in use
                true
            }
        }
    } else {
        false
    }
}

fn path_present(path: &str) -> bool {
    match rustix::fs::lstat(path) {
        Ok(_) => true,
        Err(Errno::NOENT) => false,
        Err(_) => true,
    }
}

fn export_display(display_name: &str) {
    let cmd = "systemctl --user import-environment DISPLAY; \
               hash dbus-update-activation-environment 2>/dev/null && \
               dbus-update-activation-environment DISPLAY";
    match Command::new("/bin/sh")
        .args(["-c", cmd])
        .env("DISPLAY", display_name)
        .spawn()
    {
        Ok(mut child) => {
            // Deliberately synchronous, like the session import in `main`:
            // autostart, and whatever it starts through the user manager, must
            // find DISPLAY already exported. Startup-only, and bounded by
            // systemctl's own bus timeout.
            if let Err(e) = child.wait() {
                tracing::warn!("Error waiting for DISPLAY import: {e}");
            }
        }
        Err(e) => tracing::warn!("Failed to import DISPLAY: {e}"),
    }
}
