//! Browser-specialist readiness checks.
//!
//! Per the architecture's browser-specialist gate: the browser image is present
//! or buildable, Chromium launches headlessly, `agent-browser` (or equivalent)
//! is available, artifact directories are writable, the network policy resolves,
//! and no external channel destinations are assigned.
//!
//! Image, Chromium-launch, and CLI checks require a Docker host with the browser
//! image, so they take an injected probe: real wiring supplies a container
//! exec / launch probe, and environments without that host (e.g. this sandbox)
//! record them as skipped. The artifact-storage and network-policy checks are
//! pure host-side logic and run everywhere.
//!
//! `CheckStatus` mirrors `assistant-runtime-docker`'s readiness status shape. The two
//! crates duplicate the small enum rather than couple to each other, honoring
//! the module dependency boundary (browser does not depend on the docker
//! runtime); the host unifies them when aggregating readiness.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::network::NetworkPolicy;

/// The outcome of one readiness check.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum CheckStatus {
    Pass,
    Fail { detail: String },
    /// Not evaluated here (e.g. requires the browser container); the caller must
    /// run it in the target environment.
    Skipped { detail: String },
}

impl CheckStatus {
    pub fn is_pass(&self) -> bool {
        matches!(self, CheckStatus::Pass)
    }

    pub fn is_blocking_failure(&self) -> bool {
        matches!(self, CheckStatus::Fail { .. })
    }
}

/// Marker for a check that requires the browser container/Docker host that is
/// not available here.
pub fn skipped_no_browser(check: &str) -> CheckStatus {
    CheckStatus::Skipped {
        detail: format!("{check} requires the browser container; run in the target environment"),
    }
}

/// Check that the browser image is present/buildable via an injected probe.
pub fn browser_image_ready(probe: impl FnOnce() -> bool) -> CheckStatus {
    if probe() {
        CheckStatus::Pass
    } else {
        CheckStatus::Fail {
            detail: "browser-specialist image is not present or buildable".to_string(),
        }
    }
}

/// Check that Chromium launches headlessly inside the container via an injected
/// probe (e.g. a `chromium --headless --dump-dom about:blank` exec).
pub fn chromium_launch_ready(probe: impl FnOnce() -> bool) -> CheckStatus {
    if probe() {
        CheckStatus::Pass
    } else {
        CheckStatus::Fail {
            detail: "Chromium did not launch headlessly in the browser container".to_string(),
        }
    }
}

/// Check that the `agent-browser` (or equivalent) CLI is available via an
/// injected probe.
pub fn agent_browser_cli_ready(probe: impl FnOnce() -> bool) -> CheckStatus {
    if probe() {
        CheckStatus::Pass
    } else {
        CheckStatus::Fail {
            detail: "agent-browser CLI is not available in the browser container".to_string(),
        }
    }
}

/// Check that every artifact/state root exists and is writable. Writability is
/// probed by creating and removing a temp file in the root.
pub fn artifact_storage_writable(roots: &[&Path]) -> CheckStatus {
    for root in roots {
        if !root.exists() {
            return CheckStatus::Fail {
                detail: format!("artifact root {} does not exist", root.display()),
            };
        }
        let probe = root.join(".claw-browser-write-probe");
        match std::fs::write(&probe, b"") {
            Ok(()) => {
                let _ = std::fs::remove_file(&probe);
            }
            Err(e) => {
                return CheckStatus::Fail {
                    detail: format!("artifact root {} is not writable: {e}", root.display()),
                };
            }
        }
    }
    CheckStatus::Pass
}

/// Check that the network policy resolves (see [`NetworkPolicy::resolves`]).
pub fn network_policy_resolves(policy: &NetworkPolicy) -> CheckStatus {
    if policy.resolves() {
        CheckStatus::Pass
    } else {
        CheckStatus::Fail {
            detail: "browser network policy does not resolve (allowlist names no domains)"
                .to_string(),
        }
    }
}

/// Check that the browser specialist has no external channel destinations. The
/// browser specialist returns results to the orchestrator only and must never
/// own an external channel destination.
pub fn no_external_destinations(destinations: &[String]) -> CheckStatus {
    if destinations.is_empty() {
        CheckStatus::Pass
    } else {
        CheckStatus::Fail {
            detail: format!(
                "browser specialist must have no external destinations; found {}",
                destinations.join(", ")
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn injected_probes_pass_and_fail() {
        assert!(browser_image_ready(|| true).is_pass());
        assert!(browser_image_ready(|| false).is_blocking_failure());
        assert!(chromium_launch_ready(|| true).is_pass());
        assert!(chromium_launch_ready(|| false).is_blocking_failure());
        assert!(agent_browser_cli_ready(|| true).is_pass());
        assert!(agent_browser_cli_ready(|| false).is_blocking_failure());
    }

    #[test]
    fn skipped_is_neither_pass_nor_blocking() {
        let s = skipped_no_browser("Chromium launch");
        assert!(!s.is_pass());
        assert!(!s.is_blocking_failure());
    }

    #[test]
    fn writable_existing_root_passes_and_missing_fails() {
        let dir = tempfile::tempdir().unwrap();
        assert!(artifact_storage_writable(&[dir.path()]).is_pass());
        assert!(
            artifact_storage_writable(&[Path::new("/no/such/browser/root")]).is_blocking_failure()
        );
    }

    #[test]
    fn network_policy_check_follows_resolves() {
        assert!(network_policy_resolves(&NetworkPolicy::deny_all()).is_pass());
        assert!(
            network_policy_resolves(&NetworkPolicy::allowlist(["example.com"])).is_pass()
        );
        assert!(
            network_policy_resolves(&NetworkPolicy::allowlist(Vec::<String>::new()))
                .is_blocking_failure()
        );
    }

    #[test]
    fn external_destinations_are_a_blocking_failure() {
        assert!(no_external_destinations(&[]).is_pass());
        assert!(
            no_external_destinations(&["slack:C123".to_string()]).is_blocking_failure()
        );
    }

    #[test]
    fn check_status_round_trips_json() {
        let status = CheckStatus::Fail { detail: "x".into() };
        let json = serde_json::to_string(&status).unwrap();
        let back: CheckStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(status, back);
    }
}
