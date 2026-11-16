//! Writes a release plan back into manifest files.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::Value;
use thiserror::Error;
use toml_edit::{DocumentMut, Item, Value as TomlValue};

use crate::plan::Release;

/// The manifest format of a project. Package names are scoped by it, because an npm
/// package and a Cargo crate can share a name without being the same project.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ManifestKind {
    PackageJson,
    CargoToml,
}

impl ManifestKind {
    fn file_name(self) -> &'static str {
        match self {
            ManifestKind::PackageJson => "package.json",
            ManifestKind::CargoToml => "Cargo.toml",
        }
    }
}

/// The manifests of one project, and the package names other manifests use for it.
#[derive(Debug, Clone)]
pub struct ApplyTarget {
    /// The format of every manifest of the project.
    pub kind: ManifestKind,
    /// The project name, as keyed in the plan.
    pub project: String,
    /// Names that dependency specs use to refer to the project. A Cargo workspace
    /// lists every member crate here.
    pub package_names: Vec<String>,
    /// Manifest files of the project. Each must be named as `kind` says.
    pub manifests: Vec<PathBuf>,
}

/// Errors raised while applying a plan.
#[derive(Debug, Error)]
pub enum ApplyError {
    #[error("I/O error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("invalid JSON in {path}: {source}")]
    Json {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("invalid TOML in {path}: {source}")]
    Toml {
        path: PathBuf,
        #[source]
        source: toml_edit::TomlError,
    },
    #[error("{path} is not a package.json or Cargo.toml")]
    UnsupportedManifest { path: PathBuf },
    #[error("{path} has no version to update, but its project is released")]
    MissingVersion { path: PathBuf },
    #[error("the plan releases {project}, which has no apply target")]
    UnknownProject { project: String },
}

/// Sets each released project's new version in its manifests, and moves exact
/// internal dependency specs to the new version.
///
/// A spec moves when it is the old version of a released project, optionally behind
/// `^`, `~`, or `=`. Protocol specs such as `workspace:` and `file:` never match.
/// `peerDependencies` are not touched. Nothing is written unless every manifest
/// was updated in memory first. The writes that follow are sequential, so an I/O
/// failure partway through can leave some files updated. Returns the files whose
/// content changed.
pub fn apply_plan(
    plan: &BTreeMap<String, Release>,
    targets: &[ApplyTarget],
) -> Result<Vec<PathBuf>, ApplyError> {
    for project in plan.keys() {
        if !targets.iter().any(|t| &t.project == project) {
            return Err(ApplyError::UnknownProject { project: project.clone() });
        }
    }
    // (kind, package name) -> (old, new) for every released project.
    let mut renames: Renames = BTreeMap::new();
    for target in targets {
        if let Some(release) = plan.get(&target.project) {
            for name in &target.package_names {
                renames.insert((target.kind, name.as_str()), (release.current.to_string(), release.next.to_string()));
            }
        }
    }

    // Path -> (original, current text). A manifest listed under several targets
    // gets every target's edits, applied one after another.
    let mut files: BTreeMap<PathBuf, (String, String)> = BTreeMap::new();
    for target in targets {
        let own = plan.get(&target.project).map(|r| r.next.to_string());
        let mut version_set = false;
        for path in &target.manifests {
            if path.file_name().and_then(|n| n.to_str()) != Some(target.kind.file_name()) {
                return Err(ApplyError::UnsupportedManifest { path: path.clone() });
            }
            let current = match files.get(path) {
                Some((_, current)) => current.clone(),
                None => std::fs::read_to_string(path)
                    .map_err(|source| ApplyError::Io { path: path.clone(), source })?,
            };
            let (updated, set) = match target.kind {
                ManifestKind::PackageJson => {
                    (update_package_json(path, &current, own.as_deref(), &renames)?, own.is_some())
                }
                ManifestKind::CargoToml => {
                    update_cargo_toml(path, &current, own.as_deref(), &renames)?
                }
            };
            version_set |= set;
            match files.get_mut(path) {
                Some(entry) => entry.1 = updated,
                None => {
                    files.insert(path.clone(), (current, updated));
                }
            }
        }
        // A project that inherits its version from a manifest outside its target
        // would be reported as released while keeping the old version.
        if own.is_some()
            && !version_set
            && let Some(path) = target.manifests.first()
        {
            return Err(ApplyError::MissingVersion { path: path.clone() });
        }
    }

    let mut changed = Vec::new();
    for (path, (original, updated)) in files {
        if updated == original {
            continue;
        }
        std::fs::write(&path, updated)
            .map_err(|source| ApplyError::Io { path: path.clone(), source })?;
        changed.push(path);
    }
    changed.sort();
    Ok(changed)
}

/// Rewrites `spec` when it names the old version, optionally behind `^`, `~`, or
/// `=`. A spec may give only the leading components, such as `0.1` or `1`. It then
/// follows the same number of leading components of the new version.
fn rewrite_spec(spec: &str, old: &str, new: &str) -> Option<String> {
    let rest = spec.trim_start_matches(['^', '~', '=']);
    let prefix = &spec[..spec.len() - rest.len()];
    if prefix.len() > 1 {
        return None;
    }
    if rest == old {
        return Some(format!("{prefix}{new}"));
    }
    // Pre-release and build parts are only matched in full.
    if old.contains(['-', '+']) || new.contains(['-', '+']) {
        return None;
    }
    let parts: Vec<&str> = rest.split('.').collect();
    let old_parts: Vec<&str> = old.split('.').collect();
    let new_parts: Vec<&str> = new.split('.').collect();
    let is_partial = parts.len() < old_parts.len()
        && parts.iter().all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
        && parts == old_parts[..parts.len()];
    is_partial.then(|| format!("{prefix}{}", new_parts[..parts.len()].join(".")))
}

type Renames<'a> = BTreeMap<(ManifestKind, &'a str), (String, String)>;

fn update_package_json(
    path: &Path,
    original: &str,
    own: Option<&str>,
    renames: &Renames,
) -> Result<String, ApplyError> {
    let mut doc: Value = serde_json::from_str(original)
        .map_err(|source| ApplyError::Json { path: path.to_path_buf(), source })?;
    if let Some(new) = own {
        match doc.get_mut("version") {
            Some(version) if version.is_string() => *version = Value::String(new.to_string()),
            _ => return Err(ApplyError::MissingVersion { path: path.to_path_buf() }),
        }
    }
    for section in ["dependencies", "devDependencies", "optionalDependencies"] {
        let Some(deps) = doc.get_mut(section).and_then(Value::as_object_mut) else { continue };
        for (name, spec) in deps.iter_mut() {
            let (Some((old, new)), Some(text)) = (renames.get(&(ManifestKind::PackageJson, name.as_str())), spec.as_str()) else {
                continue;
            };
            if let Some(rewritten) = rewrite_spec(text, old, new) {
                *spec = Value::String(rewritten);
            }
        }
    }
    let rendered = render_json(&doc, original)
        .map_err(|source| ApplyError::Json { path: path.to_path_buf(), source })?;
    // Reserialising can differ in layout from the author's. Leave a file whose
    // content did not change exactly as it is.
    let unchanged = serde_json::from_str::<Value>(original).is_ok_and(|v| v == doc);
    Ok(if unchanged { original.to_string() } else { rendered })
}

/// Serialises with the indentation and final newline the file already uses.
fn render_json(doc: &Value, original: &str) -> Result<String, serde_json::Error> {
    let indent: Vec<u8> = original
        .lines()
        .find(|line| line.trim_start().starts_with('"') && line.starts_with([' ', '\t']))
        .map(|line| line.bytes().take_while(|b| *b == b' ' || *b == b'\t').collect())
        .unwrap_or_else(|| b"  ".to_vec());
    let mut out = Vec::new();
    let formatter = serde_json::ser::PrettyFormatter::with_indent(&indent);
    let mut serializer = serde_json::Serializer::with_formatter(&mut out, formatter);
    serde::Serialize::serialize(doc, &mut serializer)?;
    let mut text = String::from_utf8(out).expect("serde_json writes UTF-8");
    if original.ends_with('\n') {
        text.push('\n');
    }
    // Newlines in the output are all structural, because serde_json escapes them
    // inside strings.
    if original.contains("\r\n") {
        text = text.replace('\n', "\r\n");
    }
    Ok(text)
}

/// Returns the new text, and whether a literal version was set in this manifest.
fn update_cargo_toml(
    path: &Path,
    original: &str,
    own: Option<&str>,
    renames: &Renames,
) -> Result<(String, bool), ApplyError> {
    let mut doc: DocumentMut = original
        .parse()
        .map_err(|source| ApplyError::Toml { path: path.to_path_buf(), source })?;
    let mut version_set = false;
    if let Some(new) = own {
        // A literal package version, or the version the members inherit.
        version_set = set_version(doc.get_mut("package"), new);
        version_set |= set_version(
            doc.get_mut("workspace")
                .and_then(Item::as_table_like_mut)
                .and_then(|w| w.get_mut("package")),
            new,
        );
    }

    let mut tables: Vec<&mut Item> = Vec::new();
    collect_dependency_tables(doc.as_item_mut(), &mut tables);
    for deps in tables {
        let Some(deps) = deps.as_table_like_mut() else { continue };
        for (key, entry) in deps.iter_mut() {
            let name = real_name(entry, key.get()).to_string();
            let Some((old, new)) = renames.get(&(ManifestKind::CargoToml, name.as_str())) else { continue };
            let version = match entry.as_value_mut() {
                Some(TomlValue::String(_)) => entry.as_value_mut(),
                _ => entry.as_table_like_mut().and_then(|t| t.get_mut("version")).and_then(Item::as_value_mut),
            };
            if let Some(value) = version
                && let Some(rewritten) = value.as_str().and_then(|s| rewrite_spec(s, old, new))
            {
                replace_string(value, &rewritten);
            }
        }
    }
    Ok((doc.to_string(), version_set))
}

/// The crate a dependency entry refers to: its `package` key when renamed.
fn real_name<'a>(entry: &'a Item, key: &'a str) -> &'a str {
    entry
        .as_table_like()
        .and_then(|t| t.get("package"))
        .and_then(Item::as_str)
        .unwrap_or(key)
}

/// Replaces a string value and keeps its surrounding whitespace and comments.
fn replace_string(value: &mut TomlValue, new: &str) {
    let decor = value.decor().clone();
    *value = TomlValue::from(new);
    *value.decor_mut() = decor;
}

/// Every dependency table of a manifest: top level, per target, and the workspace's.
fn collect_dependency_tables<'a>(root: &'a mut Item, out: &mut Vec<&'a mut Item>) {
    const KINDS: [&str; 3] = ["dependencies", "dev-dependencies", "build-dependencies"];
    let Some(table) = root.as_table_like_mut() else { return };
    for (key, item) in table.iter_mut() {
        match key.get() {
            k if KINDS.contains(&k) => out.push(item),
            "workspace" => {
                if let Some(deps) = item.as_table_like_mut().and_then(|w| w.get_mut("dependencies")) {
                    out.push(deps);
                }
            }
            "target" => {
                let Some(targets) = item.as_table_like_mut() else { continue };
                for (_, spec) in targets.iter_mut() {
                    let Some(spec) = spec.as_table_like_mut() else { continue };
                    for (kind, deps) in spec.iter_mut() {
                        if KINDS.contains(&kind.get()) {
                            out.push(deps);
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

/// Replaces the literal string `version` of a table, and reports whether it did.
fn set_version(table: Option<&mut Item>, new: &str) -> bool {
    let value = table
        .and_then(Item::as_table_like_mut)
        .and_then(|t| t.get_mut("version"))
        .and_then(Item::as_value_mut)
        .filter(|v| v.is_str());
    match value {
        Some(value) => {
            replace_string(value, new);
            true
        }
        None => false,
    }
}
