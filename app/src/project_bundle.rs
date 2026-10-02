//! Project bundle persistence: the lifecycle around a directory of authored
//! source plus its Spool metadata.
//!
//! A bundle pairs `lamine.yaml` (Spool structural metadata) with the
//! HTML/CSS/SVG files the metadata binds to. This module owns exactly two
//! transitions:
//!
//! ```text
//!   project on disk -> lamine.yaml -> resolve references -> validate
//!                   -> PersistentDocument
//!   PersistentDocument -> lamine.yaml -> disk
//! ```
//!
//! Two rules constrain everything here:
//!
//! 1. **Authored source is canonical.** Loading reads source; it never
//!    parses, normalizes, or rewrites it. Saving writes `lamine.yaml` and
//!    nothing else. There is deliberately no code path in this module that
//!    emits HTML, CSS, or SVG, so "source bytes survive a metadata-only
//!    save" holds by absence of capability rather than by a check that a
//!    later edit could weaken.
//! 2. **Ambiguity is reported, never guessed.** A binding that resolves to
//!    zero or to more than one authored element is an error. So is a
//!    reference that escapes the bundle root.
//!
//! This module does not define the project's directory layout. `lamine.yaml`
//! is located at the bundle root; the `file` of each source binding is
//! resolved relative to that root, so a project may use `pages/`, `styles/`,
//! `assets/`, or anything else without this module being changed.

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Component, Path, PathBuf};

use crate::source_document::{
    HtmlSource, LamineStructure, ModelError, NodeId, PersistentDocument,
};

/// The one file Spool owns inside a project directory.
pub const METADATA_FILE: &str = "lamine.yaml";

/// A failure while loading or saving a project bundle.
///
/// Each variant names the specific file or reference at fault so a caller can
/// act on it without re-reading the directory.
#[derive(Debug)]
pub enum BundleError {
    /// No `lamine.yaml` at the bundle root.
    MissingMetadata { root: PathBuf },
    /// `lamine.yaml` was read but is not valid version-1 metadata.
    MalformedMetadata { path: PathBuf, source: ModelError },
    /// A source binding named a file that is not present in the bundle.
    MissingSource { file: String, referenced_by: NodeId },
    /// A source binding pointed outside the bundle root or was otherwise not
    /// a usable in-bundle reference.
    InvalidSourceReference { file: String, referenced_by: NodeId },
    /// A source binding did not resolve to exactly one authored element.
    UnresolvedBinding {
        id: NodeId,
        file: String,
        selector: String,
        matches: usize,
    },
    /// Reading or writing failed. Metadata replacement is failure-safe, so
    /// this never leaves a partially written `lamine.yaml` behind.
    Io { path: PathBuf, source: std::io::Error },
}

impl fmt::Display for BundleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingMetadata { root } => {
                write!(f, "no {METADATA_FILE} found in project root {}", root.display())
            }
            Self::MalformedMetadata { path, source } => {
                write!(f, "{} is not valid version-1 metadata: {source}", path.display())
            }
            Self::MissingSource {
                file,
                referenced_by,
            } => write!(
                f,
                "node {} binds to {file}, which is missing from the project",
                referenced_by.as_str()
            ),
            Self::InvalidSourceReference {
                file,
                referenced_by,
            } => write!(
                f,
                "node {} binds to {file:?}, which is not a valid in-project reference",
                referenced_by.as_str()
            ),
            Self::UnresolvedBinding {
                id,
                file,
                selector,
                matches,
            } => write!(
                f,
                "node {} binds to {selector} in {file}, which matched {matches} elements (expected exactly 1)",
                id.as_str()
            ),
            Self::Io { path, source } => {
                write!(f, "{} could not be accessed: {source}", path.display())
            }
        }
    }
}

impl std::error::Error for BundleError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::MalformedMetadata { source, .. } => Some(source),
            Self::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

/// A loaded project bundle: validated metadata plus the exact authored bytes
/// each node binds to.
pub struct ProjectBundle {
    root: PathBuf,
    pub document: PersistentDocument,
}

impl ProjectBundle {
    /// Locate `lamine.yaml` for a project root without reading it.
    ///
    /// Returns `MissingMetadata` when the root holds no metadata file. This is
    /// the only place the metadata filename is decided.
    pub fn locate_metadata(root: impl AsRef<Path>) -> Result<PathBuf, BundleError> {
        let root = root.as_ref();
        let path = root.join(METADATA_FILE);
        if path.is_file() {
            Ok(path)
        } else {
            Err(BundleError::MissingMetadata {
                root: root.to_path_buf(),
            })
        }
    }

    /// Load a bundle from a project root.
    ///
    /// Reads and validates `lamine.yaml`, resolves every source binding to a
    /// file inside the bundle, and confirms each binding resolves to exactly
    /// one authored element. Returns an error rather than a partially
    /// resolved document, so a caller never observes a bundle it did not
    /// validate.
    pub fn load(root: impl AsRef<Path>) -> Result<Self, BundleError> {
        let root = root.as_ref().to_path_buf();
        let metadata_path = Self::locate_metadata(&root)?;
        let metadata = read(&metadata_path)?;
        let structure = LamineStructure::from_yaml(&metadata)
            .map_err(|source| BundleError::MalformedMetadata {
                path: metadata_path.clone(),
                source,
            })?;

        // Read each bound file once, in deterministic order. A reference that
        // escapes the root is rejected before any file is opened.
        let mut authored: BTreeMap<String, String> = BTreeMap::new();
        for node in &structure.nodes {
            if authored.contains_key(&node.source.file) {
                continue;
            }
            let path = resolve_within_root(&root, &node.source.file)
                .ok_or_else(|| BundleError::InvalidSourceReference {
                    file: node.source.file.clone(),
                    referenced_by: node.id.clone(),
                })?;
            // A missing file is the common case and deserves its own
            // actionable variant rather than a generic IO error.
            let contents = std::fs::read_to_string(&path)
                .map_err(|source| {
                    if source.kind() == std::io::ErrorKind::NotFound {
                        BundleError::MissingSource {
                            file: node.source.file.clone(),
                            referenced_by: node.id.clone(),
                        }
                    } else {
                        BundleError::Io { path, source }
                    }
                })?;
            authored.insert(node.source.file.clone(), contents);
        }

        // Every node must bind to exactly one authored element. Zero is a
        // dangling binding; more than one is ambiguous. Neither is guessed.
        for node in &structure.nodes {
            let source = HtmlSource {
                file: node.source.file.clone(),
                contents: authored[&node.source.file].clone(),
            };
            let matches = source.binding_occurrences(node);
            if matches != 1 {
                return Err(BundleError::UnresolvedBinding {
                    id: node.id.clone(),
                    file: node.source.file.clone(),
                    selector: node.source.selector.clone(),
                    matches,
                });
            }
        }

        Ok(Self {
            root,
            document: PersistentDocument {
                structure,
                sources: authored.into_iter().collect(),
            },
        })
    }

    /// The project root this bundle was loaded from.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Serialize the metadata exactly as it would be written to disk.
    ///
    /// Deterministic: the same document always produces the same bytes, which
    /// is what makes metadata-only diffs meaningful. Validates first, so this
    /// fails rather than emitting metadata that could not be reloaded.
    pub fn metadata_yaml(&self) -> Result<String, BundleError> {
        self.document
            .structure
            .to_yaml()
            .map_err(|source| BundleError::MalformedMetadata {
                path: self.root.join(METADATA_FILE),
                source,
            })
    }

    /// Persist metadata only, replacing `lamine.yaml` failure-safely.
    ///
    /// Writes to a temporary file in the same directory and renames it over
    /// the target, so a failure part-way through leaves the previous
    /// `lamine.yaml` intact rather than a truncated one. Authored files are
    /// never written, so HTML/CSS/SVG keep their exact bytes and any files
    /// Spool does not understand are left untouched.
    pub fn save(&self) -> Result<(), BundleError> {
        let encoded = self.metadata_yaml()?;
        let target = self.root.join(METADATA_FILE);
        write_atomically(&target, encoded.as_bytes())
    }
}

/// Resolve `reference` inside `root`, rejecting anything that escapes it.
///
/// Absolute paths and `..` traversal are rejected: a binding names a file in
/// this project, not anywhere on the filesystem.
fn resolve_within_root(root: &Path, reference: &str) -> Option<PathBuf> {
    let relative = Path::new(reference);
    if relative.is_absolute() {
        return None;
    }
    let mut resolved = root.to_path_buf();
    for component in relative.components() {
        match component {
            Component::Normal(part) => resolved.push(part),
            Component::CurDir => {}
            // Escapes the root, or is a root/prefix we already rejected.
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    Some(resolved)
}

fn read(path: &Path) -> Result<String, BundleError> {
    std::fs::read_to_string(path).map_err(|source| BundleError::Io {
        path: path.to_path_buf(),
        source,
    })
}

/// Replace `target` with `bytes` without ever exposing a partial write.
fn write_atomically(target: &Path, bytes: &[u8]) -> Result<(), BundleError> {
    use std::io::Write;

    let directory = target.parent().unwrap_or_else(|| Path::new("."));
    // A sibling temp file keeps the rename on one filesystem, which is what
    // makes the replacement atomic.
    let temporary = directory.join(format!(
        ".{}.tmp-{}",
        target
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| METADATA_FILE.into()),
        std::process::id()
    ));

    let write_result = (|| -> std::io::Result<()> {
        let mut file = std::fs::File::create(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        Ok(())
    })();
    if let Err(source) = write_result {
        let _ = std::fs::remove_file(&temporary);
        return Err(BundleError::Io {
            path: target.to_path_buf(),
            source,
        });
    }

    if let Err(source) = std::fs::rename(&temporary, target) {
        let _ = std::fs::remove_file(&temporary);
        return Err(BundleError::Io {
            path: target.to_path_buf(),
            source,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source_document::{RenameNode, SemanticHistory};

    /// Copy a checked-in fixture into a scratch directory so tests that save,
    /// corrupt, or delete files never modify the committed fixture.
    ///
    /// Tests run in parallel, so each call gets a unique directory; sharing
    /// one would let concurrent tests delete each other's files.
    fn scratch(fixture: &str) -> PathBuf {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures").join(fixture);
        let unique = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let scratch = std::env::temp_dir().join(format!(
            "spool-bundle-{}-{unique}-{}",
            fixture.file_name().unwrap().to_string_lossy(),
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&scratch);
        copy_tree(&fixture, &scratch).expect("copy fixture into scratch dir");
        scratch
    }

    fn copy_tree(from: &Path, to: &Path) -> std::io::Result<()> {
        std::fs::create_dir_all(to)?;
        for entry in std::fs::read_dir(from)? {
            let entry = entry?;
            let target = to.join(entry.file_name());
            if entry.file_type()?.is_dir() {
                copy_tree(&entry.path(), &target)?;
            } else {
                std::fs::copy(entry.path(), &target)?;
            }
        }
        Ok(())
    }

    fn fixture_root(fixture: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures")
            .join(fixture)
    }

    #[test]
    fn valid_bundle_loads_with_identity_hierarchy_and_bindings_intact() {
        let bundle = ProjectBundle::load(fixture_root("landing")).expect("fixture loads");
        let nodes = &bundle.document.structure.nodes;

        assert_eq!(nodes.len(), 3);
        let root = &nodes[0];
        assert_eq!(root.id.as_str(), "spool-frame-root");
        assert_eq!(root.name, "Landing Frame");
        assert_eq!(root.kind, "frame");
        assert_eq!(root.parent, None);
        assert_eq!(
            root.children,
            vec![
                NodeId::new("spool-text-headline").unwrap(),
                NodeId::new("spool-cta-primary").unwrap(),
            ]
        );

        let headline = &nodes[1];
        assert_eq!(headline.name, "Headline");
        assert_eq!(headline.kind, "text");
        assert_eq!(headline.parent.as_ref(), Some(&root.id));

        // Authored content is loaded as bytes, not reconstructed.
        assert_eq!(bundle.document.sources.keys().collect::<Vec<_>>(), ["index.html"]);
        assert!(bundle.document.sources["index.html"]
            .contains("Design in source, structure in Spool"));
    }

    #[test]
    fn bundle_layout_is_not_frozen_pages_styles_and_assets_resolve() {
        // Nothing in the loader knows these directory names; they come only
        // from each node's own `file` binding.
        let bundle = ProjectBundle::load(fixture_root("nested")).expect("nested fixture loads");
        let files: Vec<&String> = bundle.document.sources.keys().collect();
        assert_eq!(files, ["pages/index.html"]);
        assert!(bundle.document.sources["pages/index.html"].contains("spool-page-home"));

        // styles/ and assets/ exist but carry no data-spool-id binding, so
        // they are deliberately not loaded: this phase resolves what
        // metadata references and does not go looking for more.
        assert!(bundle.root().join("styles/site.css").is_file());
        assert!(bundle.root().join("assets/logo.svg").is_file());
    }

    #[test]
    fn missing_metadata_is_reported_with_the_root_that_was_searched() {
        let dir = scratch("landing");
        std::fs::remove_file(dir.join(METADATA_FILE)).unwrap();
        match ProjectBundle::load(&dir) {
            Err(BundleError::MissingMetadata { root }) => assert_eq!(root, dir),
            other => panic!("expected MissingMetadata, got {other:?}", other = describe(other)),
        }
        // locate_metadata is the single source of truth for the filename.
        assert!(matches!(
            ProjectBundle::locate_metadata(&dir),
            Err(BundleError::MissingMetadata { .. })
        ));
    }

    #[test]
    fn malformed_metadata_is_reported_against_the_metadata_path() {
        let dir = scratch("landing");
        std::fs::write(dir.join(METADATA_FILE), "version: 2\nnodes: []\n").unwrap();
        match ProjectBundle::load(&dir) {
            Err(BundleError::MalformedMetadata { path, .. }) => {
                assert_eq!(path, dir.join(METADATA_FILE));
            }
            other => panic!("expected MalformedMetadata, got {other:?}", other = describe(other)),
        }
    }

    #[test]
    fn duplicate_and_dangling_metadata_are_rejected_by_existing_validation() {
        let dir = scratch("landing");
        let original = std::fs::read_to_string(dir.join(METADATA_FILE)).unwrap();

        // Duplicate id.
        let duplicated_id = original.replacen(
            "id: \"spool-text-headline\"",
            "id: \"spool-frame-root\"",
            1,
        );
        std::fs::write(dir.join(METADATA_FILE), &duplicated_id).unwrap();
        assert!(matches!(
            ProjectBundle::load(&dir),
            Err(BundleError::MalformedMetadata {
                source: ModelError::DuplicateId(_),
                ..
            })
        ));

        // Dangling child reference.
        let dangling = original.replacen(
            "children: [\"spool-text-headline\", \"spool-cta-primary\"]",
            "children: [\"spool-text-headline\", \"spool-ghost\"]",
            1,
        );
        std::fs::write(dir.join(METADATA_FILE), &dangling).unwrap();
        assert!(matches!(
            ProjectBundle::load(&dir),
            Err(BundleError::MalformedMetadata {
                source: ModelError::MissingNode(_),
                ..
            })
        ));
    }

    #[test]
    fn missing_source_file_names_the_node_and_the_file() {
        let dir = scratch("landing");
        std::fs::remove_file(dir.join("index.html")).unwrap();
        match ProjectBundle::load(&dir) {
            Err(BundleError::MissingSource { file, referenced_by }) => {
                assert_eq!(file, "index.html");
                assert_eq!(referenced_by.as_str(), "spool-frame-root");
            }
            other => panic!("expected MissingSource, got {other:?}", other = describe(other)),
        }
    }

    #[test]
    fn dangling_binding_reports_zero_matches_and_ambiguity_is_not_guessed() {
        let dir = scratch("landing");
        let html_path = dir.join("index.html");
        let original = std::fs::read_to_string(&html_path).unwrap();

        // Dangling: the authored identity marker is gone.
        std::fs::write(&html_path, original.replace("spool-cta-primary", "renamed-away")).unwrap();
        match ProjectBundle::load(&dir) {
            Err(BundleError::UnresolvedBinding { id, matches, .. }) => {
                assert_eq!(id.as_str(), "spool-cta-primary");
                assert_eq!(matches, 0);
            }
            other => panic!("expected UnresolvedBinding, got {other:?}", other = describe(other)),
        }

        // Ambiguous: the same identity appears twice.
        std::fs::write(&html_path, format!("{original}\n<!-- {original} -->")).unwrap();
        match ProjectBundle::load(&dir) {
            Err(BundleError::UnresolvedBinding { id, matches, .. }) => {
                assert_eq!(id.as_str(), "spool-frame-root");
                assert_eq!(matches, 2);
            }
            other => panic!("expected UnresolvedBinding, got {other:?}", other = describe(other)),
        }
    }

    #[test]
    fn references_escaping_the_project_root_are_rejected() {
        for escape in ["../outside.html", "/etc/passwd", "pages/../../outside.html"] {
            let dir = scratch("landing");
            let metadata = std::fs::read_to_string(dir.join(METADATA_FILE)).unwrap();
            let escaped = metadata.replacen("\"index.html\"", &format!("\"{escape}\""), 1);
            std::fs::write(dir.join(METADATA_FILE), &escaped).unwrap();
            assert!(
                matches!(
                    ProjectBundle::load(&dir),
                    Err(BundleError::InvalidSourceReference { .. })
                ),
                "expected {escape:?} to be rejected as an in-project reference"
            );
        }
    }

    #[test]
    fn save_load_round_trip_preserves_the_document() {
        let dir = scratch("landing");
        let mut bundle = ProjectBundle::load(&dir).expect("scratch loads");

        bundle.document.structure.nodes[1].name = "Hero headline".into();
        bundle.save().expect("save succeeds");

        let reloaded = ProjectBundle::load(&dir).expect("reload after save");
        assert_eq!(reloaded.document.structure, bundle.document.structure);
        assert_eq!(reloaded.document.sources, bundle.document.sources);
    }

    #[test]
    fn metadata_only_save_leaves_source_bytes_unchanged() {
        let dir = scratch("landing");
        let bundle = ProjectBundle::load(&dir).expect("scratch loads");

        let html_before = std::fs::read(dir.join("index.html")).unwrap();
        let css_before = std::fs::read(dir.join("styles.css")).unwrap();
        let metadata_before = std::fs::read_to_string(dir.join(METADATA_FILE)).unwrap();

        let mut edited = bundle;
        edited.document.structure.nodes[1].name = "Hero headline".into();
        edited.save().expect("save");

        assert_eq!(std::fs::read(dir.join("index.html")).unwrap(), html_before);
        assert_eq!(std::fs::read(dir.join("styles.css")).unwrap(), css_before);

        let metadata_after = std::fs::read_to_string(dir.join(METADATA_FILE)).unwrap();
        assert_ne!(metadata_after, metadata_before);
        assert!(metadata_after.contains("Hero headline"));
    }

    #[test]
    fn save_preserves_unrelated_and_unknown_project_files() {
        let dir = scratch("landing");
        // Files Spool knows nothing about must survive a metadata save.
        std::fs::write(dir.join("notes.md"), "scratch notes\n").unwrap();
        std::fs::create_dir_all(dir.join("assets")).unwrap();
        let unrelated = dir.join("assets/photo.png");
        std::fs::write(&unrelated, [0x89, b'P', b'N', b'G']).unwrap();

        let bundle = ProjectBundle::load(&dir).expect("scratch loads");
        bundle.save().expect("save");

        assert_eq!(
            std::fs::read_to_string(dir.join("notes.md")).unwrap(),
            "scratch notes\n"
        );
        assert_eq!(std::fs::read(&unrelated).unwrap(), [0x89, b'P', b'N', b'G']);
    }

    #[test]
    fn metadata_serialization_is_deterministic() {
        let dir = scratch("landing");
        let bundle = ProjectBundle::load(&dir).expect("scratch loads");

        // Same document in, same bytes out, every time.
        assert_eq!(bundle.metadata_yaml().unwrap(), bundle.metadata_yaml().unwrap());

        // And a reload of those bytes reproduces the same document.
        let first = bundle.metadata_yaml().unwrap();
        std::fs::write(dir.join(METADATA_FILE), &first).unwrap();
        let reloaded = ProjectBundle::load(&dir).expect("reload");
        assert_eq!(reloaded.metadata_yaml().unwrap(), first);
        assert_eq!(reloaded.document.structure, bundle.document.structure);
    }

    #[test]
    fn invalid_metadata_is_never_written_and_the_old_file_survives() {
        let dir = scratch("landing");
        let metadata_before = std::fs::read(dir.join(METADATA_FILE)).unwrap();

        let mut bundle = ProjectBundle::load(&dir).expect("scratch loads");
        // Collides with an existing name; to_yaml validates before writing.
        bundle.document.structure.nodes[1].name = "Landing Frame".into();
        assert!(matches!(
            bundle.save(),
            Err(BundleError::MalformedMetadata { .. })
        ));
        assert_eq!(
            std::fs::read(dir.join(METADATA_FILE)).unwrap(),
            metadata_before
        );
        assert!(!has_temp_files(&dir), "no temporary file should be left behind");
    }

    #[test]
    #[cfg(unix)]
    fn metadata_is_replaced_atomically_rather_than_truncated_in_place() {
        use std::os::unix::fs::MetadataExt;

        let dir = scratch("landing");
        let target = dir.join(METADATA_FILE);
        let mut bundle = ProjectBundle::load(&dir).expect("scratch loads");

        // Atomic replacement swaps in a new file, so the target's identity
        // changes. An in-place rewrite would keep the same inode, which is
        // the observable difference between the two strategies.
        let before = std::fs::metadata(&target).unwrap().ino();

        bundle.document.structure.nodes[1].name = "Hero headline".into();
        bundle.save().expect("save succeeds");

        let after = std::fs::metadata(&target).unwrap().ino();
        assert_ne!(
            before, after,
            "metadata should be replaced by rename, not rewritten in place"
        );
        assert!(ProjectBundle::load(&dir).is_ok(), "the swapped-in file must be valid");
    }

    #[test]
    #[cfg(unix)]
    fn unwritable_directory_fails_the_save_without_debris() {
        use std::os::unix::fs::PermissionsExt;

        let dir = scratch("landing");
        let metadata_before = std::fs::read_to_string(dir.join(METADATA_FILE)).unwrap();
        let bundle = ProjectBundle::load(&dir).expect("scratch loads");

        // Remove write permission from the directory so the temporary file
        // cannot be created. The bundle is loaded first, so the failure is
        // attributable to saving and not to loading.
        let original_mode = std::fs::metadata(&dir).unwrap().permissions().mode();
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o500)).unwrap();

        let outcome = bundle.save();

        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(original_mode)).unwrap();

        assert!(
            matches!(outcome, Err(BundleError::Io { .. })),
            "saving into an unwritable directory must fail"
        );
        assert_eq!(
            std::fs::read_to_string(dir.join(METADATA_FILE)).unwrap(),
            metadata_before,
            "a failed save must not corrupt existing metadata"
        );
        assert!(
            !has_temp_files(&dir),
            "failed save must clean up its temporary file"
        );
    }

    #[test]
    #[cfg(unix)]
    fn a_failing_replacement_keeps_the_target_and_removes_the_temporary() {
        let dir = scratch("landing");
        let metadata_before = std::fs::read_to_string(dir.join(METADATA_FILE)).unwrap();
        let mut bundle = ProjectBundle::load(&dir).expect("scratch loads");
        bundle.document.structure.nodes[1].name = "Hero headline".into();
        let edited_yaml = bundle.metadata_yaml().expect("serialize edited metadata");

        // Occupy the target path with a directory. The temporary file is
        // still created successfully, so this exercises the replacement step
        // specifically: the save must fail there and clean up after itself,
        // rather than leaving a temporary file or a half-written target.
        let target = dir.join(METADATA_FILE);
        std::fs::remove_file(&target).unwrap();
        std::fs::create_dir(&target).unwrap();

        assert!(
            matches!(bundle.save(), Err(BundleError::Io { .. })),
            "replacing metadata with a directory must fail"
        );
        assert!(
            target.is_dir(),
            "a failed replacement must not have clobbered the target path"
        );
        assert!(
            !has_temp_files(&dir),
            "a failed replacement must clean up its temporary file"
        );

        // Once the obstruction is gone the same save succeeds and writes
        // exactly the metadata it had computed before the failure.
        std::fs::remove_dir(&target).unwrap();
        std::fs::write(&target, &metadata_before).unwrap();
        bundle.save().expect("save succeeds once the target is a file");
        assert_eq!(
            std::fs::read_to_string(&target).unwrap(),
            edited_yaml,
            "the retried save must produce the metadata it had computed"
        );
    }

    #[test]
    fn semantic_rename_survives_save_undo_and_reload() {
        let dir = scratch("landing");
        let css_before = std::fs::read(dir.join("styles.css")).unwrap();
        let mut bundle = ProjectBundle::load(&dir).expect("scratch loads");
        let mut history = SemanticHistory::default();

        let rename = RenameNode {
            id: NodeId::new("spool-text-headline").unwrap(),
            before: "Headline".into(),
            after: "Hero headline".into(),
        };
        assert!(history.commit(&mut bundle.document, rename).unwrap());
        bundle.save().expect("save renamed metadata");

        assert!(history.undo(&mut bundle.document).unwrap());
        bundle.save().expect("save after undo");
        let reloaded = ProjectBundle::load(&dir).expect("reload");
        assert_eq!(reloaded.document.structure.nodes[1].name, "Headline");

        // CSS is not in metadata and was byte-identical throughout.
        assert_eq!(std::fs::read(dir.join("styles.css")).unwrap(), css_before);
    }

    #[test]
    fn document_sources_never_duplicate_metadata_or_unreferenced_files() {
        let bundle = ProjectBundle::load(fixture_root("landing")).expect("fixture loads");
        // Only files a node binds to are loaded, and metadata is not among
        // them: sources are authored bytes, not a second copy of lamine.yaml.
        assert_eq!(bundle.document.sources.keys().collect::<Vec<_>>(), ["index.html"]);
        assert!(!bundle.document.sources.contains_key(METADATA_FILE));
        assert!(!bundle.document.sources.contains_key("styles.css"));
    }

    /// Name the outcome of a load without printing a whole document.
    fn describe(error: Result<ProjectBundle, BundleError>) -> &'static str {
        match error {
            Ok(_) => "Ok",
            Err(_) => "Err",
        }
    }

    fn has_temp_files(dir: &Path) -> bool {
        std::fs::read_dir(dir)
            .map(|entries| {
                entries.filter_map(Result::ok).any(|entry| {
                    entry
                        .file_name()
                        .to_string_lossy()
                        .starts_with(".lamine.yaml.tmp")
                })
            })
            .unwrap_or(false)
    }
}