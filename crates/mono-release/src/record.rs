//! Change records: markdown files with a frontmatter map from project to bump level.

use std::collections::BTreeMap;

use crate::ReleaseError;

/// How much a version moves. Ordered from smallest to largest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Bump {
    Patch,
    Minor,
    Major,
}

/// One change record: the bump each project receives, and a summary for the changelog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangeRecord {
    pub bumps: BTreeMap<String, Bump>,
    pub summary: String,
}

/// Parses a change record. It starts with a frontmatter block between `---` lines,
/// which maps project names to `major`, `minor`, or `patch`. The rest is the summary.
pub fn parse_record(text: &str) -> Result<ChangeRecord, ReleaseError> {
    let rest = text
        .strip_prefix("---\n")
        .ok_or_else(|| ReleaseError::InvalidRecord { message: "missing the opening --- line".into() })?;
    let end = rest
        .find("\n---")
        .ok_or_else(|| ReleaseError::InvalidRecord { message: "missing the closing --- line".into() })?;
    let front = &rest[..end];
    let summary = rest[end + "\n---".len()..].trim_start_matches('\n').to_string();

    let raw: BTreeMap<String, String> = serde_yaml_ng::from_str(front)
        .map_err(|e| ReleaseError::InvalidRecord { message: e.to_string() })?;
    let mut bumps = BTreeMap::new();
    for (project, value) in raw {
        bumps.insert(project, parse_bump(&value)?);
    }
    Ok(ChangeRecord { bumps, summary })
}

fn parse_bump(value: &str) -> Result<Bump, ReleaseError> {
    match value {
        "patch" => Ok(Bump::Patch),
        "minor" => Ok(Bump::Minor),
        "major" => Ok(Bump::Major),
        other => Err(ReleaseError::InvalidBump { value: other.to_string() }),
    }
}
