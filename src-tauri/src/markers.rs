//! "This already happened" markers kept in the user cache. They are written by
//! the unprivileged app process only, never by root.

use std::path::{Path, PathBuf};

use crate::*;

/// Sub directory of the user cache dir used for "this already happened" markers.
pub(crate) const CACHE_SUBDIR: &str = "archdroid";

pub(crate) fn cache_dir() -> PathBuf {
    let mut dir = dirs::cache_dir().unwrap_or_else(std::env::temp_dir);
    dir.push(CACHE_SUBDIR);
    dir
}

pub(crate) fn marker_file(name: &str) -> PathBuf {
    cache_dir().join(name)
}

/// Persist a marker so a finished install is still known after a restart.
/// This is only called from the unprivileged app process, after a root command
/// has completed successfully; root never writes into the user cache.
/// A read-only cache directory must never break a status check, so callers
/// that only refresh a marker may ignore the error; install completion reports it.
pub(crate) fn write_marker(marker: &Path) -> std::io::Result<()> {
    if let Some(parent) = marker.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(marker, MARKER_PENDING)
}

/// An install script exited 0 but no live check has confirmed the result yet.
pub(crate) const MARKER_PENDING: &str = "pending\n";
/// A live check against the running container confirmed the add-on.
pub(crate) const MARKER_VERIFIED: &str = "verified\n";

/// Marker state as the offline status path should report it.
#[derive(Debug, PartialEq)]
pub(crate) enum MarkerState {
    Absent,
    Pending,
    Verified,
}

pub(crate) fn marker_state(marker: &Path) -> MarkerState {
    match std::fs::read_to_string(marker) {
        Ok(text) if text == MARKER_VERIFIED => MarkerState::Verified,
        // Older builds wrote "installed\n" without a live check: treat as pending.
        Ok(_) => MarkerState::Pending,
        Err(_) => MarkerState::Absent,
    }
}

/// Status for an add-on whose container cannot be asked right now.
pub(crate) fn offline_marker_report(marker: &Path, what: &str) -> Option<CommandResult> {
    match marker_state(marker) {
        MarkerState::Verified => Some(CommandResult::report(
            "installed",
            &format!(
                "{what} installed (verified on the last live check). Start Waydroid to re-check."
            ),
        )),
        MarkerState::Pending => Some(CommandResult::report(
            "pending",
            &format!("{what} install finished but is not verified yet. Start Waydroid to verify."),
        )),
        MarkerState::Absent => None,
    }
}

/// Record a live-check confirmation; failure only costs the offline shortcut.
pub(crate) fn refresh_marker(marker: &Path) {
    let write = || -> std::io::Result<()> {
        if let Some(parent) = marker.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(marker, MARKER_VERIFIED)
    };
    if let Err(error) = write() {
        log::warn!("could not refresh {}: {error}", marker.display());
    }
}
