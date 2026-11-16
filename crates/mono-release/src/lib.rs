//! Release planning: change records, and the versions they imply.

mod apply;
mod plan;
mod record;

pub use apply::{apply_plan, ApplyError, ApplyTarget, ManifestKind};
pub use plan::{compute_plan, Release};
pub use record::{parse_record, Bump, ChangeRecord};

use thiserror::Error;

/// Errors raised while reading records or planning a release.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ReleaseError {
    #[error("change record is malformed: {message}")]
    InvalidRecord { message: String },
    #[error("unknown bump {value}: use major, minor, or patch")]
    InvalidBump { value: String },
    #[error("change record names project {name}, which has no version")]
    UnknownProject { name: String },
}
