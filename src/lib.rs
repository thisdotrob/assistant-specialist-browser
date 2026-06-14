//! Browser-automation specialist profile: artifact/state path confinement,
//! network policy, the versioned profile/capability declaration, and the
//! browser-specialist readiness gates.
//!
//! This crate owns host-side browser-specialist policy. It does not depend on
//! `claw-runtime-docker`: the host runs the container and feeds Chromium/CLI
//! launch probes into the readiness gates here. Path confinement, network
//! policy, and the artifact policy are pure host-side logic with full unit
//! coverage; the Chromium-launch and image/CLI checks require the browser
//! container and so take injected probes (skipped in this sandbox).

pub mod artifact;
pub mod error;
pub mod network;
pub mod profile;
pub mod readiness;
pub mod spec;

pub use artifact::{
    approve_artifact, artifact_path, confine_to_approved_roots, ArtifactKind, ArtifactPolicy,
};
pub use error::ArtifactError;
pub use network::{NetworkMode, NetworkPolicy};
pub use profile::{
    BrowserSpecialistProfile, BROWSER_PROFILE_ID, BROWSER_PROFILE_KIND, BROWSER_PROFILE_VERSION,
    DEFAULT_MAX_ARTIFACT_BYTES, DEFAULT_MAX_CONCURRENT_JOBS, DEFAULT_MAX_SPECIALISTS,
};
pub use readiness::{
    agent_browser_cli_ready, artifact_storage_writable, browser_image_ready, chromium_launch_ready,
    network_policy_resolves, no_external_destinations, skipped_no_browser, CheckStatus,
};
pub use spec::{
    browser_specialist_spec, network_guardrail, BROWSER_DESCRIPTION, BROWSER_GROUP_SLUG,
    BROWSER_IMAGE_REPOSITORY, BROWSER_MAX_TURNS, BROWSER_ROUTE_NAME,
};

pub const MODULE_ID: &str = "claw-specialist-browser";
pub const MODULE_VERSION: &str = env!("CARGO_PKG_VERSION");
