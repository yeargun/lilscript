use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};

use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::config::{DependencyConfig, ProjectConfig};

pub const LILSCRIPT_ABI_VERSION: u32 = 1;
pub const LOCKFILE_VERSION: u32 = 1;
pub const EFFECTS_FILE_VERSION: u32 = 1;
pub const EFFECTS_FILE_NAME: &str = "lilscript.effects.toml";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackageEffectSummary {
    pub functions: BTreeMap<String, FunctionEffectMeta>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FunctionEffectMeta {
    pub pure: bool,
    pub mutated_parameters: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PackageEffectsFile {
    version: u32,
    functions: BTreeMap<String, FunctionEffectMeta>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackageLock {
    pub version: u32,
    pub compiler_abi: u32,
    pub root_dependencies: BTreeMap<String, String>,
    pub packages: Vec<LockedPackage>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LockedPackage {
    pub name: String,
    pub version: String,
    pub abi: u32,
    pub source: String,
    pub entry: String,
    pub checksum: String,
    pub dependencies: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageResolver {
    root: PathBuf,
    root_dependencies: BTreeSet<String>,
    packages: BTreeMap<String, LockedPackage>,
    package_roots: Vec<(PathBuf, String)>,
    effects: BTreeMap<String, PackageEffectSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageError {
    pub path: PathBuf,
    pub message: String,
}

impl PackageError {
    fn new(path: impl Into<PathBuf>, message: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            message: message.into(),
        }
    }
}

impl std::fmt::Display for PackageError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.path.display(), self.message)
    }
}

impl std::error::Error for PackageError {}

/// Effect summaries live in `lilscript.effects.toml` beside the package root and are not part of
/// `lilscript.lock`, so existing lockfiles remain compatible across ABI 1 packages.
pub fn write_package_effects(
    package_root: &Path,
    summary: &PackageEffectSummary,
) -> Result<(), PackageError> {
    let path = package_root.join(EFFECTS_FILE_NAME);
    let encoded = toml::to_string(&PackageEffectsFile {
        version: EFFECTS_FILE_VERSION,
        functions: summary.functions.clone(),
    })
    .map_err(|error| PackageError::new(&path, format!("failed to encode effects file: {error}")))?;
    fs::write(&path, encoded).map_err(|error| {
        PackageError::new(&path, format!("failed to write effects file: {error}"))
    })?;
    Ok(())
}

pub fn load_package_effects(
    package_root: &Path,
) -> Result<Option<PackageEffectSummary>, PackageError> {
    let path = package_root.join(EFFECTS_FILE_NAME);
    if !path.is_file() {
        return Ok(None);
    }
    let source = fs::read_to_string(&path).map_err(|error| {
        PackageError::new(&path, format!("failed to read effects file: {error}"))
    })?;
    let file = toml::from_str::<PackageEffectsFile>(&source)
        .map_err(|error| PackageError::new(&path, format!("invalid effects file: {error}")))?;
    if file.version != EFFECTS_FILE_VERSION {
        return Err(PackageError::new(
            &path,
            format!(
                "effects file version {} is unsupported; expected {}",
                file.version, EFFECTS_FILE_VERSION
            ),
        ));
    }
    Ok(Some(PackageEffectSummary {
        functions: file.functions,
    }))
}

pub fn write_lockfile(config: &ProjectConfig) -> Result<PathBuf, PackageError> {
    let root = config_root(config)?;
    let lock = build_lockfile(config)?;
    let encoded = toml::to_string(&lock)
        .map_err(|error| PackageError::new(&root, format!("failed to encode lockfile: {error}")))?;
    let path = root.join("lilscript.lock");
    fs::write(&path, encoded)
        .map_err(|error| PackageError::new(&path, format!("failed to write lockfile: {error}")))?;
    if config.package.is_some() {
        if let Some(summary) = effect_summary_for_package_root(&root, config)? {
            write_package_effects(&root, &summary)?;
        }
    }
    for package in &lock.packages {
        let package_root = root.join(&package.source);
        if let Some(summary) = effect_summary_for_entry(&package_root.join(&package.entry))? {
            write_package_effects(&package_root, &summary)?;
        }
    }
    Ok(path)
}

fn effect_summary_for_package_root(
    root: &Path,
    config: &ProjectConfig,
) -> Result<Option<PackageEffectSummary>, PackageError> {
    let Some(package) = &config.package else {
        return Ok(None);
    };
    effect_summary_for_entry(&root.join(&package.entry))
}

fn effect_summary_for_entry(entry: &Path) -> Result<Option<PackageEffectSummary>, PackageError> {
    if !entry.is_file() {
        return Ok(None);
    }
    let source = fs::read_to_string(entry).map_err(|error| {
        PackageError::new(entry, format!("failed to read package entry: {error}"))
    })?;
    let arena = bumpalo::Bump::new();
    let program = crate::parser::parse_source(&arena, &source).map_err(|error| {
        PackageError::new(entry, format!("failed to parse package entry: {error}"))
    })?;
    let semantics = crate::semantic::analyze(&program).map_err(|error| {
        PackageError::new(entry, format!("failed to analyze package entry: {error}"))
    })?;
    let module = crate::lower::lower_to_control_flow(&program, &semantics).map_err(|error| {
        PackageError::new(entry, format!("failed to lower package entry: {error}"))
    })?;
    Ok(Some(crate::optimizer::summarize_module_effects(&module)))
}

pub fn load_package_resolver(
    config: &ProjectConfig,
) -> Result<Option<PackageResolver>, PackageError> {
    if config.dependencies.is_empty() {
        return Ok(None);
    }
    let root = config_root(config)?;
    let path = root.join("lilscript.lock");
    let source = fs::read_to_string(&path).map_err(|error| {
        PackageError::new(
            &path,
            format!("failed to read lockfile: {error}; run `lilscript <entry> --write-lock`"),
        )
    })?;
    let actual = toml::from_str::<PackageLock>(&source)
        .map_err(|error| PackageError::new(&path, format!("invalid lockfile: {error}")))?;
    let expected = build_lockfile(config)?;
    if actual != expected {
        return Err(PackageError::new(
            &path,
            "lockfile is stale or dependency contents changed; run `lilscript <entry> --write-lock`",
        ));
    }
    let root_dependencies = actual.root_dependencies.keys().cloned().collect();
    let packages = actual
        .packages
        .into_iter()
        .map(|package| (package.name.clone(), package))
        .collect::<BTreeMap<_, _>>();
    let mut package_roots = packages
        .values()
        .map(|package| {
            root.join(&package.source)
                .canonicalize()
                .map(|path| (path, package.name.clone()))
                .map_err(|error| {
                    PackageError::new(
                        root.join(&package.source),
                        format!("cannot resolve locked package: {error}"),
                    )
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    package_roots.sort_unstable_by(|(left, _), (right, _)| {
        right
            .components()
            .count()
            .cmp(&left.components().count())
            .then_with(|| left.cmp(right))
    });
    let mut effects = BTreeMap::new();
    for (package_root, name) in &package_roots {
        if let Some(summary) = load_package_effects(package_root)? {
            effects.insert(name.clone(), summary);
        }
    }
    Ok(Some(PackageResolver {
        root,
        root_dependencies,
        packages,
        package_roots,
        effects,
    }))
}

impl PackageResolver {
    pub fn effects(&self, package_name: &str) -> Option<&PackageEffectSummary> {
        self.effects.get(package_name)
    }

    pub fn resolve(&self, importer: &Path, specifier: &str) -> Result<PathBuf, PackageError> {
        let (name, subpath) = specifier
            .split_once('/')
            .map_or((specifier, None), |(name, path)| (name, Some(path)));
        let declaring_package = self
            .package_roots
            .iter()
            .find(|(root, _)| importer.starts_with(root))
            .map(|(_, name)| name);
        let declared = declaring_package.map_or_else(
            || self.root_dependencies.contains(name),
            |declaring| {
                self.packages
                    .get(declaring)
                    .is_some_and(|package| package.dependencies.contains_key(name))
            },
        );
        if !declared {
            let owner = declaring_package.map_or("root package", String::as_str);
            return Err(PackageError::new(
                importer,
                format!("package `{name}` is not declared by {owner}"),
            ));
        }
        let package = self.packages.get(name).ok_or_else(|| {
            PackageError::new(
                &self.root,
                format!("package `{name}` is not present in lilscript.lock"),
            )
        })?;
        let package_root = self
            .root
            .join(&package.source)
            .canonicalize()
            .map_err(|error| {
                PackageError::new(
                    self.root.join(&package.source),
                    format!("cannot resolve locked package: {error}"),
                )
            })?;
        let relative = subpath.unwrap_or(&package.entry);
        let requested = package_root.join(relative);
        let requested = if requested.extension().is_none() {
            requested.with_extension("lil")
        } else {
            requested
        };
        let resolved = requested.canonicalize().map_err(|error| {
            PackageError::new(
                &requested,
                format!("cannot resolve package module `{specifier}`: {error}"),
            )
        })?;
        if !resolved.starts_with(&package_root) {
            return Err(PackageError::new(
                &resolved,
                format!("package module `{specifier}` escapes its package root"),
            ));
        }
        if resolved
            .extension()
            .and_then(|extension| extension.to_str())
            != Some("lil")
        {
            return Err(PackageError::new(
                &resolved,
                "package modules must use the `.lil` extension",
            ));
        }
        Ok(resolved)
    }
}

fn build_lockfile(config: &ProjectConfig) -> Result<PackageLock, PackageError> {
    let root = config_root(config)?;
    let mut packages = BTreeMap::<String, LockedPackage>::new();
    let mut visiting = BTreeSet::new();
    for (name, dependency) in &config.dependencies {
        visit_dependency(name, dependency, &root, &root, &mut packages, &mut visiting)?;
    }
    Ok(PackageLock {
        version: LOCKFILE_VERSION,
        compiler_abi: LILSCRIPT_ABI_VERSION,
        root_dependencies: config
            .dependencies
            .iter()
            .map(|(name, dependency)| (name.clone(), dependency.version.clone()))
            .collect(),
        packages: packages.into_values().collect(),
    })
}

fn visit_dependency(
    requested_name: &str,
    dependency: &DependencyConfig,
    declaring_root: &Path,
    project_root: &Path,
    packages: &mut BTreeMap<String, LockedPackage>,
    visiting: &mut BTreeSet<PathBuf>,
) -> Result<(), PackageError> {
    let package_root = declaring_root
        .join(&dependency.path)
        .canonicalize()
        .map_err(|error| {
            PackageError::new(
                declaring_root.join(&dependency.path),
                format!("cannot resolve dependency `{requested_name}`: {error}"),
            )
        })?;
    if visiting.contains(&package_root) {
        return Err(PackageError::new(
            package_root.join("lilscript.toml"),
            format!("cyclic package dependency involving `{requested_name}`"),
        ));
    }
    let config_path = package_root.join("lilscript.toml");
    let mut package_config = read_package_config(&config_path)?;
    package_config.config_dir = Some(package_root.clone());
    package_config
        .validate()
        .map_err(|message| PackageError::new(&config_path, message))?;
    let metadata = package_config.package.as_ref().ok_or_else(|| {
        PackageError::new(
            &config_path,
            format!("dependency `{requested_name}` has no `[package]` metadata"),
        )
    })?;
    if metadata.name != requested_name {
        return Err(PackageError::new(
            &config_path,
            format!(
                "dependency key `{requested_name}` does not match package name `{}`",
                metadata.name
            ),
        ));
    }
    let version = Version::parse(&metadata.version)
        .map_err(|error| PackageError::new(&config_path, format!("invalid version: {error}")))?;
    let requirement = VersionReq::parse(&dependency.version).map_err(|error| {
        PackageError::new(
            &config_path,
            format!("invalid version requirement for `{requested_name}`: {error}"),
        )
    })?;
    if !requirement.matches(&version) {
        return Err(PackageError::new(
            &config_path,
            format!(
                "package `{requested_name}` version {version} does not satisfy {}",
                dependency.version
            ),
        ));
    }
    if metadata.abi != dependency.abi || metadata.abi != LILSCRIPT_ABI_VERSION {
        return Err(PackageError::new(
            &config_path,
            format!(
                "package `{requested_name}` ABI {} is incompatible with requested/compiler ABI {}",
                metadata.abi, dependency.abi
            ),
        ));
    }
    let entry = package_root
        .join(&metadata.entry)
        .canonicalize()
        .map_err(|error| {
            PackageError::new(
                package_root.join(&metadata.entry),
                format!("cannot resolve package entry: {error}"),
            )
        })?;
    if !entry.starts_with(&package_root)
        || entry.extension().and_then(|extension| extension.to_str()) != Some("lil")
    {
        return Err(PackageError::new(
            &entry,
            "package entry must be a `.lil` file inside the package root",
        ));
    }

    let source = relative_path(project_root, &package_root)?;
    let source = normalized_path(&source);
    let entry = normalized_path(
        entry
            .strip_prefix(&package_root)
            .expect("entry is inside root"),
    );
    let locked = LockedPackage {
        name: metadata.name.clone(),
        version: metadata.version.clone(),
        abi: metadata.abi,
        source,
        entry,
        checksum: package_checksum(&package_root)?,
        dependencies: package_config
            .dependencies
            .iter()
            .map(|(name, dependency)| (name.clone(), dependency.version.clone()))
            .collect(),
    };
    if let Some(previous) = packages.get(requested_name) {
        if previous != &locked {
            return Err(PackageError::new(
                &config_path,
                format!("package `{requested_name}` resolves to conflicting versions or sources"),
            ));
        }
        return Ok(());
    }
    visiting.insert(package_root.clone());
    packages.insert(requested_name.to_string(), locked);
    for (name, dependency) in &package_config.dependencies {
        visit_dependency(
            name,
            dependency,
            &package_root,
            project_root,
            packages,
            visiting,
        )?;
    }
    visiting.remove(&package_root);
    Ok(())
}

fn read_package_config(path: &Path) -> Result<ProjectConfig, PackageError> {
    let source = fs::read_to_string(path).map_err(|error| {
        PackageError::new(path, format!("failed to read package manifest: {error}"))
    })?;
    toml::from_str(&source)
        .map_err(|error| PackageError::new(path, format!("invalid package manifest: {error}")))
}

fn package_checksum(root: &Path) -> Result<String, PackageError> {
    let mut files = Vec::new();
    collect_package_files(root, root, &mut files)?;
    files.sort_unstable_by(|left, right| left.0.cmp(&right.0));
    let mut hasher = Sha256::new();
    for (relative, path) in files {
        let contents = fs::read(&path).map_err(|error| {
            PackageError::new(&path, format!("failed to hash package source: {error}"))
        })?;
        hasher.update((relative.len() as u64).to_le_bytes());
        hasher.update(relative.as_bytes());
        hasher.update((contents.len() as u64).to_le_bytes());
        hasher.update(contents);
    }
    let digest = hasher.finalize();
    let mut encoded = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write as _;
        write!(encoded, "{byte:02x}").expect("writing a digest to String cannot fail");
    }
    Ok(format!("sha256:{encoded}"))
}

fn collect_package_files(
    root: &Path,
    directory: &Path,
    files: &mut Vec<(String, PathBuf)>,
) -> Result<(), PackageError> {
    let entries = fs::read_dir(directory).map_err(|error| {
        PackageError::new(directory, format!("failed to enumerate package: {error}"))
    })?;
    for entry in entries {
        let entry = entry.map_err(|error| {
            PackageError::new(directory, format!("failed to enumerate package: {error}"))
        })?;
        let path = entry.path();
        let file_type = entry.file_type().map_err(|error| {
            PackageError::new(&path, format!("failed to inspect package file: {error}"))
        })?;
        if file_type.is_symlink() {
            return Err(PackageError::new(
                &path,
                "package source trees may not contain symbolic links",
            ));
        }
        if file_type.is_dir() {
            collect_package_files(root, &path, files)?;
        } else if path.file_name().and_then(|name| name.to_str()) == Some("lilscript.toml")
            || path.extension().and_then(|extension| extension.to_str()) == Some("lil")
        {
            let relative = path
                .strip_prefix(root)
                .expect("enumerated path is inside root");
            files.push((normalized_path(relative), path));
        }
    }
    Ok(())
}

fn config_root(config: &ProjectConfig) -> Result<PathBuf, PackageError> {
    config.config_dir.clone().ok_or_else(|| {
        PackageError::new(
            "lilscript.toml",
            "package dependencies require a file-backed project configuration",
        )
    })
}

fn normalized_path(path: &Path) -> String {
    path.components()
        .filter_map(|component| match component {
            Component::Normal(value) => value.to_str(),
            Component::ParentDir => Some(".."),
            Component::CurDir => Some("."),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn relative_path(from: &Path, to: &Path) -> Result<PathBuf, PackageError> {
    let from = from.components().collect::<Vec<_>>();
    let to = to.components().collect::<Vec<_>>();
    let shared = from
        .iter()
        .zip(&to)
        .take_while(|(left, right)| left == right)
        .count();
    if shared == 0 {
        return Err(PackageError::new(
            to.iter().collect::<PathBuf>(),
            "package source is on a different filesystem root",
        ));
    }
    let mut path = PathBuf::new();
    for _ in shared..from.len() {
        path.push("..");
    }
    for component in &to[shared..] {
        path.push(component.as_os_str());
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_portable_relative_paths() {
        assert_eq!(
            relative_path(Path::new("/work/app"), Path::new("/work/packages/math")).unwrap(),
            PathBuf::from("../packages/math")
        );
    }

    #[test]
    fn rejects_undeclared_transitive_package_access() {
        let mut packages = BTreeMap::new();
        packages.insert(
            "direct".to_string(),
            LockedPackage {
                name: "direct".to_string(),
                version: "1.0.0".to_string(),
                abi: LILSCRIPT_ABI_VERSION,
                source: "packages/direct".to_string(),
                entry: "lib.lil".to_string(),
                checksum: "sha256:test".to_string(),
                dependencies: BTreeMap::new(),
            },
        );
        packages.insert(
            "hidden".to_string(),
            LockedPackage {
                name: "hidden".to_string(),
                version: "1.0.0".to_string(),
                abi: LILSCRIPT_ABI_VERSION,
                source: "packages/hidden".to_string(),
                entry: "lib.lil".to_string(),
                checksum: "sha256:test".to_string(),
                dependencies: BTreeMap::new(),
            },
        );
        let resolver = PackageResolver {
            root: PathBuf::from("/work/app"),
            root_dependencies: BTreeSet::from(["direct".to_string()]),
            packages,
            package_roots: vec![
                (
                    PathBuf::from("/work/app/packages/direct"),
                    "direct".to_string(),
                ),
                (
                    PathBuf::from("/work/app/packages/hidden"),
                    "hidden".to_string(),
                ),
            ],
            effects: BTreeMap::new(),
        };

        let error = resolver
            .resolve(Path::new("/work/app/packages/direct/src"), "hidden")
            .unwrap_err();
        assert!(error.message.contains("not declared by direct"));

        let error = resolver
            .resolve(Path::new("/work/app/src"), "hidden")
            .unwrap_err();
        assert!(error.message.contains("not declared by root package"));
    }

    #[test]
    fn package_effects_roundtrip_beside_package_root() {
        let directory = std::env::temp_dir().join(format!(
            "lilscript-package-effects-test-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let summary = PackageEffectSummary {
            functions: BTreeMap::from([
                (
                    "add".to_string(),
                    FunctionEffectMeta {
                        pure: true,
                        mutated_parameters: Vec::new(),
                    },
                ),
                (
                    "push".to_string(),
                    FunctionEffectMeta {
                        pure: false,
                        mutated_parameters: vec![0],
                    },
                ),
            ]),
        };
        write_package_effects(&directory, &summary).unwrap();
        let loaded = load_package_effects(&directory).unwrap().unwrap();
        assert_eq!(loaded, summary);
        assert!(load_package_effects(&directory.join("missing"))
            .unwrap()
            .is_none());
        std::fs::remove_dir_all(directory).unwrap();
    }
}
