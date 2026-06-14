//! Errors for browser-specialist artifact handling.

use std::fmt;
use std::path::PathBuf;

use crate::artifact::ArtifactKind;

/// Why an artifact write target was rejected.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ArtifactError {
    /// The candidate path used `..`, an absolute prefix, or was empty — it
    /// could escape its root.
    Escape { path: PathBuf },
    /// The candidate path is not under any profile-approved root.
    OutsideApprovedRoots { path: PathBuf },
    /// The artifact kind is not in the profile's allowed set.
    DisallowedKind { kind: ArtifactKind },
    /// The artifact exceeds the profile's maximum size.
    TooLarge { size: u64, max: u64 },
}

impl fmt::Display for ArtifactError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ArtifactError::Escape { path } => {
                write!(f, "artifact path {} would escape its root", path.display())
            }
            ArtifactError::OutsideApprovedRoots { path } => write!(
                f,
                "artifact path {} is not under any profile-approved root",
                path.display()
            ),
            ArtifactError::DisallowedKind { kind } => {
                write!(f, "artifact kind {} is not allowed by profile policy", kind.as_str())
            }
            ArtifactError::TooLarge { size, max } => {
                write!(f, "artifact size {size} exceeds profile maximum {max}")
            }
        }
    }
}

impl std::error::Error for ArtifactError {}
