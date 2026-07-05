#![allow(clippy::unwrap_used)]

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use typst::diag::FileError;
use typst::syntax::package::{PackageSpec, PackageVersion};
use typst::syntax::{FileId, RootedPath, VirtualPath, VirtualRoot};

use super::{
    BytesProvider, FileProvider, ProviderCtx, local_package_file_id, resolve_path, root_file_id,
};

#[test]
fn bytes_provider_serves_registered_file() {
    let id = root_file_id("main.typ").unwrap();
    let provider = BytesProvider::new().with_file(id, *b"= Embedded");
    let ctx = ProviderCtx {
        root: Path::new("/virtual"),
        overrides: &HashMap::new(),
    };
    let bytes = provider.read(id, &ctx).unwrap();
    assert_eq!(bytes.as_slice(), b"= Embedded");
}

#[test]
fn bytes_provider_missing_file_errors() {
    let id = root_file_id("main.typ").unwrap();
    let provider = BytesProvider::new();
    let ctx = ProviderCtx {
        root: Path::new("/virtual"),
        overrides: &HashMap::new(),
    };
    assert!(matches!(
        provider.read(id, &ctx),
        Err(FileError::NotFound(_))
    ));
}

#[test]
fn root_file_id_builds_project_scoped_id() {
    let id = root_file_id("a/b.typ").unwrap();
    assert!(matches!(id.root(), VirtualRoot::Project));
    assert_eq!(id.vpath().get_without_slash(), "a/b.typ");
}

#[test]
fn root_file_id_rejects_escaping_vpath() {
    assert!(root_file_id("..").is_err());
}

#[test]
fn local_package_file_id_builds_package_scoped_id() {
    let id = local_package_file_id("mindtape", "0.2.0", "lib.typ").unwrap();
    let VirtualRoot::Package(spec) = id.root() else {
        panic!("expected a package-rooted id");
    };
    assert_eq!(spec.namespace.as_str(), "local");
    assert_eq!(spec.name.as_str(), "mindtape");
    assert_eq!(
        spec.version,
        PackageVersion {
            major: 0,
            minor: 2,
            patch: 0
        }
    );
    assert_eq!(id.vpath().get_without_slash(), "lib.typ");
}

#[test]
fn local_package_file_id_rejects_bad_version() {
    assert!(local_package_file_id("mindtape", "not-a-version", "lib.typ").is_err());
}

#[test]
fn local_package_file_id_rejects_escaping_vpath() {
    assert!(local_package_file_id("mindtape", "0.2.0", "..").is_err());
}

#[test]
fn resolve_path_joins_project_vpath_onto_root() {
    let id = root_file_id("a/b.typ").unwrap();
    let ctx = ProviderCtx {
        root: Path::new("/proj"),
        overrides: &HashMap::new(),
    };
    assert_eq!(
        resolve_path(id, &ctx).unwrap(),
        PathBuf::from("/proj/a/b.typ")
    );
}

#[test]
fn resolve_path_package_override_wins_over_platform_dir() {
    let id = local_package_file_id("mindtape", "0.2.0", "lib.typ").unwrap();
    let mut overrides = HashMap::new();
    overrides.insert("mindtape".to_owned(), PathBuf::from("/repo/lib"));
    let ctx = ProviderCtx {
        root: Path::new("/proj"),
        overrides: &overrides,
    };
    assert_eq!(
        resolve_path(id, &ctx).unwrap(),
        PathBuf::from("/repo/lib/lib.typ")
    );
}

#[test]
fn resolve_path_local_package_without_override_uses_platform_dir() {
    let id = local_package_file_id("mindtape", "0.2.0", "lib.typ").unwrap();
    let ctx = ProviderCtx {
        root: Path::new("/proj"),
        overrides: &HashMap::new(),
    };
    let path = resolve_path(id, &ctx).unwrap();
    assert!(path.ends_with(Path::new("typst/packages/local/mindtape/0.2.0/lib.typ")));
}

#[test]
fn resolve_path_rejects_non_local_namespace_without_override() {
    let spec = PackageSpec {
        namespace: "preview".into(),
        name: "mindtape".into(),
        version: PackageVersion {
            major: 0,
            minor: 2,
            patch: 0,
        },
    };
    let id = FileId::new(RootedPath::new(
        VirtualRoot::Package(spec),
        VirtualPath::new("lib.typ").unwrap(),
    ));
    let ctx = ProviderCtx {
        root: Path::new("/proj"),
        overrides: &HashMap::new(),
    };
    assert!(matches!(
        resolve_path(id, &ctx),
        Err(FileError::NotFound(_))
    ));
}
