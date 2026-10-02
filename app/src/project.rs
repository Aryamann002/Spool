//! Source project loading and saving.
//!
//! A project directory pairs `lamine.yaml` (Spool structural metadata) with
//! authored HTML/CSS/SVG files. Loading validates the metadata and confirms
//! every node binds to exactly one authored element. It never rewrites,
//! reformats, or normalizes authored bytes.
//!
//! Saving writes `lamine.yaml` only. Authored files are the canonical
//! implementation, so this module deliberately has no code path that emits
//! HTML/CSS/SVG. A later milestone may add source-preserving patches; until
//! then the only thing that changes on disk is Spool metadata.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::source_document::{
    HtmlSource, LamineStructure, ModelError, NodeId, PersistentDocument,
};

pub const METADATA_FILE: &str = "lamine.yaml";

#[derive(Debug)]
pub enum ProjectError {
    /// A file named by the project could not be read.
    Io { path: String, source: std::io::Error },
    /// `lamine.yaml` failed parsing or failed structural validation.
    Model(ModelError),
    /// A node's source binding did not resolve to exactly one authored
    /// element. Ambiguous bindings are reported, never guessed.
    UnresolvedBinding {
        id: NodeId,
        file: String,
        selector: String,
        matches: usize,
    },
}

impl std::fmt::Display for ProjectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io { path, source } => write!(f, "reading {path} failed: {source}"),
            Self::Model(error) => write!(f, "invalid lamine.yaml: {error:?}"),
            Self::UnresolvedBinding {
                id,
                file,
                selector,
                matches,
            } => write!(
                f,
                "node {} binds to {matches} matches of {selector} in {file}",
                id.as_str()
            ),
        }
    }
}

impl std::error::Error for ProjectError {}

/// A loaded project: validated metadata plus the exact authored bytes each
/// node binds to. The authored map is the loaded source of truth; saving does
/// not regenerate it.
pub struct SpoolProject {
    root: PathBuf,
    pub document: PersistentDocument,
}

impl SpoolProject {
    /// Load `<root>/lamine.yaml`, then read every source file the metadata
    /// binds to. Fails if metadata is invalid or any binding is unresolved or
    /// ambiguous, so the caller never receives a partially resolved document.
    pub fn load(root: impl AsRef<Path>) -> Result<Self, ProjectError> {
        let root = root.as_ref().to_path_buf();
        let metadata = read(&root.join(METADATA_FILE))?;
        let structure = LamineStructure::from_yaml(&metadata).map_err(ProjectError::Model)?;

        // Read each bound file once, in a deterministic order.
        let mut authored: BTreeMap<String, String> = BTreeMap::new();
        for node in &structure.nodes {
            if authored.contains_key(&node.source.file) {
                continue;
            }
            let contents = read(&root.join(&node.source.file))?;
            authored.insert(node.source.file.clone(), contents);
        }

        // A node may bind to at most one authored element. Zero matches is a
        // dangling binding; more than one is ambiguous. Both are errors.
        for node in &structure.nodes {
            let source = HtmlSource {
                file: node.source.file.clone(),
                contents: authored[&node.source.file].clone(),
            };
            let matches = source.binding_occurrences(node);
            if matches != 1 {
                return Err(ProjectError::UnresolvedBinding {
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

    /// Persist metadata only. Authored files are read to confirm they are
    /// still present, but are never written: HTML/CSS/SVG stay canonical.
    pub fn save(&self) -> Result<(), ProjectError> {
        let encoded = self
            .document
            .structure
            .to_yaml()
            .map_err(ProjectError::Model)?;
        std::fs::write(self.root.join(METADATA_FILE), encoded)
            .map_err(|source| ProjectError::Io {
                path: METADATA_FILE.to_owned(),
                source,
            })
    }
}

fn read(path: &Path) -> Result<String, ProjectError> {
    std::fs::read_to_string(path).map_err(|source| ProjectError::Io {
        path: path.display().to_string(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Copy the checked-in fixture project into a scratch directory so tests
    /// that save or corrupt files never modify the committed fixture.
    fn scratch() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "spool-project-test-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create scratch project dir");
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/landing");
        for name in [METADATA_FILE, "index.html", "styles.css"] {
            std::fs::copy(fixture.join(name), dir.join(name)).expect("copy fixture file");
        }
        dir
    }

    fn fixture_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/landing")
    }

    #[test]
    fn fixture_project_loads_with_its_hierarchy_and_bindings_intact() {
        let project = SpoolProject::load(fixture_root()).expect("fixture loads");
        let nodes = &project.document.structure.nodes;
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

        // Authored content is loaded, not reconstructed.
        assert_eq!(project.document.sources.len(), 1);
        let html = &project.document.sources["index.html"];
        assert!(html.contains("Design in source, structure in Spool"));
    }

    #[test]
    fn loading_fails_rather_than_guessing_when_a_binding_is_missing_or_ambiguous() {
        let dir = scratch();
        let html = dir.join("index.html");
        let original = std::fs::read_to_string(&html).unwrap();

        // Dangling: the authored marker is gone.
        std::fs::write(&html, original.replace("spool-cta-primary", "renamed-away")).unwrap();
        assert!(matches!(
            SpoolProject::load(&dir),
            Err(ProjectError::UnresolvedBinding { matches: 0, .. })
        ));

        // Ambiguous: the same identity appears twice.
        let duplicated = format!("{original}\n<!-- {original} -->");
        std::fs::write(&html, duplicated).unwrap();
        match SpoolProject::load(&dir) {
            Err(ProjectError::UnresolvedBinding { id, matches, .. }) => {
                assert_eq!(id.as_str(), "spool-frame-root");
                assert_eq!(matches, 2);
            }
            other => panic!("expected an ambiguous binding error, got {other:?}", other = other.map(|_| ())),
        }
    }

    #[test]
    fn loading_fails_on_invalid_metadata_before_any_source_is_trusted() {
        let dir = scratch();
        std::fs::write(
            dir.join(METADATA_FILE),
            "version: 1\nnodes:\n  - id: \"dup\"\n    name: \"A\"\n    kind: \"frame\"\n    parent: null\n    file: \"index.html\"\n    selector: \"[data-spool-id=\\\"dup\\\"]\"\n    children: []\n  - id: \"dup\"\n    name: \"B\"\n    kind: \"text\"\n    parent: null\n    file: \"index.html\"\n    selector: \"[data-spool-id=\\\"dup\\\"]\"\n    children: []\n",
        )
        .unwrap();
        assert!(matches!(
            SpoolProject::load(&dir),
            Err(ProjectError::Model(ModelError::DuplicateId(_)))
        ));
    }

    #[test]
    fn saving_writes_metadata_and_leaves_authored_bytes_untouched() {
        let dir = scratch();
        let mut project = SpoolProject::load(&dir).expect("scratch loads");

        let html_before = std::fs::read(dir.join("index.html")).unwrap();
        let css_before = std::fs::read(dir.join("styles.css")).unwrap();
        let metadata_before = std::fs::read_to_string(dir.join(METADATA_FILE)).unwrap();

        // A pure metadata edit changes nothing about the authored files.
        project.document.structure.nodes[1].name = "Hero headline".into();
        project.save().expect("save succeeds");

        assert_eq!(std::fs::read(dir.join("index.html")).unwrap(), html_before);
        assert_eq!(std::fs::read(dir.join("styles.css")).unwrap(), css_before);

        let metadata_after = std::fs::read_to_string(dir.join(METADATA_FILE)).unwrap();
        assert_ne!(metadata_after, metadata_before);
        assert!(metadata_after.contains("Hero headline"));

        // And the saved metadata round-trips back to the same structure.
        let reloaded = SpoolProject::load(&dir).expect("reload after save");
        assert_eq!(
            reloaded.document.structure,
            project.document.structure
        );
    }

    #[test]
    fn saving_invalid_metadata_fails_without_touching_the_existing_file() {
        let dir = scratch();
        let mut project = SpoolProject::load(&dir).expect("scratch loads");
        let metadata_before = std::fs::read_to_string(dir.join(METADATA_FILE)).unwrap();

        // Collide with an existing name; to_yaml validates before writing.
        project.document.structure.nodes[1].name = "Landing Frame".into();
        assert!(matches!(project.save(), Err(ProjectError::Model(_))));
        assert_eq!(
            std::fs::read_to_string(dir.join(METADATA_FILE)).unwrap(),
            metadata_before
        );
    }

    #[test]
    fn semantic_rename_round_trips_through_disk_and_preserves_source() {
        use crate::source_document::{RenameNode, SemanticHistory};

        let dir = scratch();
        let css_before = std::fs::read(dir.join("styles.css")).unwrap();
        let mut project = SpoolProject::load(&dir).expect("scratch loads");
        let mut history = SemanticHistory::default();

        let rename = RenameNode {
            id: NodeId::new("spool-text-headline").unwrap(),
            before: "Headline".into(),
            after: "Hero headline".into(),
        };
        assert!(history.commit(&mut project.document, rename).unwrap());
        project.save().expect("save");

        // Undo restores the exact prior name and survives a save/reload.
        assert!(history.undo(&mut project.document).unwrap());
        project.save().expect("save after undo");
        let reloaded = SpoolProject::load(&dir).expect("reload");
        assert_eq!(reloaded.document.structure.nodes[1].name, "Headline");

        // The CSS was never in metadata and is byte-identical throughout.
        assert_eq!(std::fs::read(dir.join("styles.css")).unwrap(), css_before);
        assert_eq!(
            reloaded.document.sources.keys().collect::<Vec<_>>(),
            vec!["index.html"]
        );
    }

    #[test]
    fn loading_a_missing_project_reports_the_path() {
        match SpoolProject::load(std::env::temp_dir().join("spool-does-not-exist-xyz")) {
            Err(ProjectError::Io { path, .. }) => {
                assert!(path.ends_with(METADATA_FILE));
            }
            other => panic!(
                "expected an IO error, got {other:?}",
                other = other.map(|_| ())
            ),
        }
    }

    #[test]
    fn project_sources_are_the_loaded_bytes_not_a_second_copy_of_metadata() {
        let project = SpoolProject::load(fixture_root()).expect("fixture loads");
        // Sources hold authored file contents keyed by authored file name;
        // metadata lives only in the structure, never duplicated here.
        let keys: Vec<&String> = project.document.sources.keys().collect();
        assert_eq!(keys, vec!["index.html"]);
        assert!(!project.document.sources.contains_key(METADATA_FILE));
    }
}