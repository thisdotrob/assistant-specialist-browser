//! Browser artifact and state path confinement.
//!
//! Browser specialists capture screenshots, traces, downloads, and DOM/text
//! captures, and persist cookies/session state. Every such write must land
//! inside a profile-approved root — never the orchestrator workspace and never
//! an arbitrary host path. Confinement is lexical (mirroring
//! `claw-session`'s attachment-path safety): we reject `..`, absolute, and
//! empty candidates, then confirm the joined path stays under an approved root.

use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::ArtifactError;

/// The artifact types a browser specialist may produce. The profile's policy
/// declares which subset is allowed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactKind {
    Screenshot,
    Trace,
    Download,
    DomCapture,
    TextCapture,
}

impl ArtifactKind {
    pub fn as_str(self) -> &'static str {
        match self {
            ArtifactKind::Screenshot => "screenshot",
            ArtifactKind::Trace => "trace",
            ArtifactKind::Download => "download",
            ArtifactKind::DomCapture => "dom_capture",
            ArtifactKind::TextCapture => "text_capture",
        }
    }
}

/// Profile-declared artifact policy plus the host-supplied approved roots.
///
/// The profile contributes `allowed_kinds` and `max_bytes`; the host fills in
/// `approved_roots` for the concrete specialist instance (its artifact and
/// browser-state directories).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArtifactPolicy {
    pub approved_roots: Vec<PathBuf>,
    pub allowed_kinds: Vec<ArtifactKind>,
    pub max_bytes: u64,
}

impl ArtifactPolicy {
    pub fn allows_kind(&self, kind: ArtifactKind) -> bool {
        self.allowed_kinds.contains(&kind)
    }
}

/// Reject candidates that could escape a root: empty, absolute, or containing
/// `..`/root/prefix components.
fn lexically_contained(candidate: &Path) -> bool {
    if candidate.as_os_str().is_empty() || candidate.is_absolute() {
        return false;
    }
    !candidate.components().any(|c| {
        matches!(
            c,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    })
}

/// Resolve a relative artifact name against a single approved root, rejecting
/// anything that could escape it.
pub fn artifact_path(root: &Path, relative: &str) -> Result<PathBuf, ArtifactError> {
    let rel = Path::new(relative);
    if !lexically_contained(rel) {
        return Err(ArtifactError::Escape {
            path: rel.to_path_buf(),
        });
    }
    let joined = root.join(rel);
    if joined.starts_with(root) {
        Ok(joined)
    } else {
        Err(ArtifactError::Escape { path: joined })
    }
}

/// Confirm an already-resolved write target stays inside one of the approved
/// roots. Rejects any path containing `..` (which could traverse out of a root
/// even if it textually starts under one).
pub fn confine_to_approved_roots(
    approved_roots: &[PathBuf],
    candidate: &Path,
) -> Result<PathBuf, ArtifactError> {
    if candidate
        .components()
        .any(|c| matches!(c, Component::ParentDir))
    {
        return Err(ArtifactError::Escape {
            path: candidate.to_path_buf(),
        });
    }
    if approved_roots.iter().any(|root| candidate.starts_with(root)) {
        Ok(candidate.to_path_buf())
    } else {
        Err(ArtifactError::OutsideApprovedRoots {
            path: candidate.to_path_buf(),
        })
    }
}

/// Validate a full artifact write against the policy: kind allowed, size within
/// limit, and target confined to an approved root.
pub fn approve_artifact(
    policy: &ArtifactPolicy,
    kind: ArtifactKind,
    size_bytes: u64,
    target: &Path,
) -> Result<PathBuf, ArtifactError> {
    if !policy.allows_kind(kind) {
        return Err(ArtifactError::DisallowedKind { kind });
    }
    if size_bytes > policy.max_bytes {
        return Err(ArtifactError::TooLarge {
            size: size_bytes,
            max: policy.max_bytes,
        });
    }
    confine_to_approved_roots(&policy.approved_roots, target)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy(roots: &[&str]) -> ArtifactPolicy {
        ArtifactPolicy {
            approved_roots: roots.iter().map(PathBuf::from).collect(),
            allowed_kinds: vec![ArtifactKind::Screenshot, ArtifactKind::TextCapture],
            max_bytes: 10 * 1024 * 1024,
        }
    }

    #[test]
    fn relative_name_joins_under_root() {
        let root = Path::new("/data/spec/browser/artifacts");
        let p = artifact_path(root, "shot.png").unwrap();
        assert_eq!(p, root.join("shot.png"));
        assert!(artifact_path(root, "sub/dir/page.html").unwrap().starts_with(root));
    }

    #[test]
    fn traversal_absolute_and_empty_names_are_rejected() {
        let root = Path::new("/data/spec/browser/artifacts");
        for bad in ["../escape.png", "a/../../escape", "/etc/passwd", ""] {
            assert!(
                matches!(artifact_path(root, bad), Err(ArtifactError::Escape { .. })),
                "{bad} not rejected"
            );
        }
    }

    #[test]
    fn target_inside_approved_root_is_confined() {
        let p = policy(&["/data/spec/browser/artifacts", "/data/spec/browser/state"]);
        assert!(
            confine_to_approved_roots(&p.approved_roots, Path::new("/data/spec/browser/state/cookies.db"))
                .is_ok()
        );
    }

    #[test]
    fn target_outside_every_approved_root_is_rejected() {
        let p = policy(&["/data/spec/browser/artifacts"]);
        let err = confine_to_approved_roots(
            &p.approved_roots,
            Path::new("/data/orchestrator/memory/secret.md"),
        )
        .unwrap_err();
        assert!(matches!(err, ArtifactError::OutsideApprovedRoots { .. }));
    }

    #[test]
    fn parent_dir_in_absolute_target_is_rejected_even_under_root() {
        let p = policy(&["/data/spec/browser/artifacts"]);
        let err = confine_to_approved_roots(
            &p.approved_roots,
            Path::new("/data/spec/browser/artifacts/../../orchestrator/x"),
        )
        .unwrap_err();
        assert!(matches!(err, ArtifactError::Escape { .. }));
    }

    #[test]
    fn approve_artifact_enforces_kind_size_and_root() {
        let p = policy(&["/data/spec/browser/artifacts"]);
        let target = Path::new("/data/spec/browser/artifacts/shot.png");
        // Happy path.
        assert!(approve_artifact(&p, ArtifactKind::Screenshot, 1024, target).is_ok());
        // Disallowed kind.
        assert!(matches!(
            approve_artifact(&p, ArtifactKind::Download, 1024, target),
            Err(ArtifactError::DisallowedKind { .. })
        ));
        // Too large.
        assert!(matches!(
            approve_artifact(&p, ArtifactKind::Screenshot, p.max_bytes + 1, target),
            Err(ArtifactError::TooLarge { .. })
        ));
        // Outside approved roots.
        assert!(matches!(
            approve_artifact(
                &p,
                ArtifactKind::Screenshot,
                1024,
                Path::new("/tmp/loose.png")
            ),
            Err(ArtifactError::OutsideApprovedRoots { .. })
        ));
    }
}
