//! The release plan: each project's next version, from the records it received.

use std::collections::BTreeMap;

use semver::Version;

use crate::record::{Bump, ChangeRecord};
use crate::ReleaseError;

/// The release of one project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Release {
    pub current: Version,
    pub next: Version,
    pub bump: Bump,
}

/// Computes the release plan.
///
/// A project named in a record gets the largest bump any record gives it. A project
/// that depends on a released project is released too, with at least a patch bump,
/// and so on transitively. `versions` holds the current version of every project.
/// `dependents` maps each project to the projects that depend on it directly.
pub fn compute_plan(
    records: &[ChangeRecord],
    versions: &BTreeMap<String, Version>,
    dependents: &BTreeMap<String, Vec<String>>,
) -> Result<BTreeMap<String, Release>, ReleaseError> {
    let mut bumps: BTreeMap<String, Bump> = BTreeMap::new();
    for record in records {
        for (project, bump) in &record.bumps {
            ensure_known(versions, project)?;
            let entry = bumps.entry(project.clone()).or_insert(*bump);
            *entry = (*entry).max(*bump);
        }
    }

    // Propagate: each released project raises its dependents to at least a patch.
    let mut queue: Vec<String> = bumps.keys().cloned().collect();
    while let Some(project) = queue.pop() {
        for dependent in dependents.get(&project).into_iter().flatten() {
            ensure_known(versions, dependent)?;
            // A project is queued the first time it gets a bump, so each project is
            // processed once and a dependency cycle cannot loop forever.
            if !bumps.contains_key(dependent) {
                bumps.insert(dependent.clone(), Bump::Patch);
                queue.push(dependent.clone());
            }
        }
    }

    let mut plan = BTreeMap::new();
    for (project, bump) in bumps {
        let current = versions[&project].clone();
        let next = bump_version(&current, bump);
        plan.insert(project, Release { current, next, bump });
    }
    Ok(plan)
}

fn ensure_known(versions: &BTreeMap<String, Version>, project: &str) -> Result<(), ReleaseError> {
    if versions.contains_key(project) {
        Ok(())
    } else {
        Err(ReleaseError::UnknownProject { name: project.to_string() })
    }
}

fn bump_version(current: &Version, bump: Bump) -> Version {
    match bump {
        Bump::Major => Version::new(current.major + 1, 0, 0),
        Bump::Minor => Version::new(current.major, current.minor + 1, 0),
        Bump::Patch => Version::new(current.major, current.minor, current.patch + 1),
    }
}
