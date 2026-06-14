//! The browser-specialist profile declaration.
//!
//! The profile is a versioned capability module: it declares its identity, the
//! artifact kinds and size limits it may produce, and its default network
//! policy. The host fills in the concrete approved roots for an instance. By
//! design the browser specialist has no external channel destinations — it
//! returns results to the orchestrator, which decides what to tell users.

use std::path::PathBuf;

use claw_agent_graph::{ProfileLimits, RegisteredProfile};
use claw_capabilities::{CapabilityDescriptor, ProfileDescriptor};
use claw_core::ProfileMetadata;

use crate::artifact::{ArtifactKind, ArtifactPolicy};
use crate::network::NetworkPolicy;

pub const BROWSER_PROFILE_ID: &str = "browser-specialist";
pub const BROWSER_PROFILE_KIND: &str = "specialist";
pub const BROWSER_PROFILE_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Default ceiling for a single captured artifact (50 MiB).
pub const DEFAULT_MAX_ARTIFACT_BYTES: u64 = 50 * 1024 * 1024;

/// At most one browser specialist per orchestrator: browsing is expensive and
/// session-stateful, so the host keeps a single long-lived instance.
pub const DEFAULT_MAX_SPECIALISTS: u32 = 1;

/// Concurrent browser-job ceiling. Deliberately over-provisioned: a single
/// long-lived specialist instance now runs its jobs on background worker
/// threads (off the serve thread), so the orchestrator can fan out many jobs
/// at once. We set this high on purpose to surface real contention under load
/// rather than guess a conservative cap; tighten only if a specific failure
/// mode actually appears.
pub const DEFAULT_MAX_CONCURRENT_JOBS: u32 = 8;

/// The browser-automation specialist profile.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BrowserSpecialistProfile {
    network: NetworkPolicy,
}

impl BrowserSpecialistProfile {
    /// A profile that browses only the given approved domains (deny-by-default).
    pub fn new(network: NetworkPolicy) -> Self {
        Self { network }
    }

    /// A profile that reaches no network until a policy is configured.
    pub fn deny_all_network() -> Self {
        Self {
            network: NetworkPolicy::deny_all(),
        }
    }

    pub fn network_policy(&self) -> &NetworkPolicy {
        &self.network
    }

    /// The artifact kinds this profile may produce.
    pub fn allowed_artifact_kinds(&self) -> Vec<ArtifactKind> {
        vec![
            ArtifactKind::Screenshot,
            ArtifactKind::Trace,
            ArtifactKind::Download,
            ArtifactKind::DomCapture,
            ArtifactKind::TextCapture,
        ]
    }

    /// Build the concrete artifact policy for an instance. The host supplies the
    /// approved roots (the instance's artifact and browser-state directories);
    /// the profile supplies the allowed kinds and size ceiling.
    pub fn artifact_policy(&self, approved_roots: Vec<PathBuf>) -> ArtifactPolicy {
        ArtifactPolicy {
            approved_roots,
            allowed_kinds: self.allowed_artifact_kinds(),
            max_bytes: self.max_artifact_bytes(),
        }
    }

    /// The browser specialist owns no external channel destinations.
    pub fn has_external_destinations(&self) -> bool {
        false
    }

    /// The per-profile creation/concurrency limits the host enforces.
    pub fn profile_limits(&self) -> ProfileLimits {
        ProfileLimits::new(DEFAULT_MAX_SPECIALISTS, DEFAULT_MAX_CONCURRENT_JOBS)
    }

    /// The size ceiling for a single returned artifact, mirrored into the
    /// agent-graph result-artifact policy so both gates agree.
    pub fn max_artifact_bytes(&self) -> u64 {
        DEFAULT_MAX_ARTIFACT_BYTES
    }

    /// Translate this profile into the plain [`RegisteredProfile`] data the agent
    /// graph admits specialists from. This is the one-way bridge into the graph:
    /// `claw-agent-graph` never depends on this crate, so the host wires the
    /// browser specialist in through this conversion. The graph then knows the
    /// profile's identity, that it is a specialist with no external
    /// destinations, and its limits.
    pub fn registered_profile(&self) -> RegisteredProfile {
        RegisteredProfile::specialist(
            BROWSER_PROFILE_ID,
            BROWSER_PROFILE_VERSION,
            self.profile_limits(),
        )
    }
}

impl ProfileDescriptor for BrowserSpecialistProfile {
    fn metadata(&self) -> ProfileMetadata {
        ProfileMetadata {
            id: BROWSER_PROFILE_ID,
            version: BROWSER_PROFILE_VERSION,
            kind: BROWSER_PROFILE_KIND,
        }
    }
}

impl CapabilityDescriptor for BrowserSpecialistProfile {
    fn id(&self) -> &'static str {
        BROWSER_PROFILE_ID
    }

    fn version(&self) -> &'static str {
        BROWSER_PROFILE_VERSION
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_metadata_is_a_specialist() {
        let profile = BrowserSpecialistProfile::deny_all_network();
        let meta = profile.metadata();
        assert_eq!(meta.id, BROWSER_PROFILE_ID);
        assert_eq!(meta.kind, "specialist");
        assert_eq!(meta.version, BROWSER_PROFILE_VERSION);
    }

    #[test]
    fn profile_has_no_external_destinations() {
        assert!(!BrowserSpecialistProfile::deny_all_network().has_external_destinations());
    }

    #[test]
    fn artifact_policy_carries_host_roots_and_profile_limits() {
        let profile = BrowserSpecialistProfile::new(NetworkPolicy::allowlist(["example.com"]));
        let roots = vec![PathBuf::from("/data/spec/browser/artifacts")];
        let policy = profile.artifact_policy(roots.clone());
        assert_eq!(policy.approved_roots, roots);
        assert_eq!(policy.max_bytes, DEFAULT_MAX_ARTIFACT_BYTES);
        assert!(policy.allows_kind(ArtifactKind::Screenshot));
    }

    #[test]
    fn capability_descriptor_matches_profile_identity() {
        let profile = BrowserSpecialistProfile::deny_all_network();
        assert_eq!(profile.id(), BROWSER_PROFILE_ID);
        assert_eq!(profile.version(), BROWSER_PROFILE_VERSION);
    }

    #[test]
    fn registered_profile_is_an_internal_specialist_with_matching_identity() {
        let profile = BrowserSpecialistProfile::new(NetworkPolicy::allowlist(["example.com"]));
        let registered = profile.registered_profile();
        assert_eq!(registered.profile_id, BROWSER_PROFILE_ID);
        assert_eq!(registered.profile_version, BROWSER_PROFILE_VERSION);
        assert!(registered.is_specialist());
        // The agent graph must agree with the profile: no external destinations.
        assert!(!registered.allows_external_destinations);
        assert_eq!(
            registered.allows_external_destinations,
            profile.has_external_destinations()
        );
    }

    #[test]
    fn registered_profile_carries_default_limits() {
        let profile = BrowserSpecialistProfile::deny_all_network();
        let limits = profile.profile_limits();
        assert_eq!(limits.max_specialists, DEFAULT_MAX_SPECIALISTS);
        assert_eq!(limits.max_concurrent_jobs, DEFAULT_MAX_CONCURRENT_JOBS);
        assert_eq!(profile.registered_profile().limits, limits);
        assert_eq!(profile.max_artifact_bytes(), DEFAULT_MAX_ARTIFACT_BYTES);
    }
}
