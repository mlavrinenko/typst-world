#![allow(clippy::unwrap_used)]

use std::collections::HashMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use typst::diag::FileError;
use typst::syntax::package::{PackageSpec, PackageVersion};
use typst::syntax::{FileId, RootedPath, VirtualPath, VirtualRoot};

use super::{
    BytesProvider, DiskProvider, FallbackProvider, FileProvider, PACKAGE_PATH_ENV, ProviderCtx,
    ScopedDiskProvider, local_package_file_id, package_root_from, platform_package_root,
    project_file_id, resolve_path, root_file_id,
};

/// A package root holding one `@local/greet:0.1.0` whose `lib.typ` reads
/// `body`. Returned so the caller keeps the tempdir alive.
fn package_root(body: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let lib = dir.path().join("local").join("greet").join("0.1.0");
    std::fs::create_dir_all(&lib).unwrap();
    std::fs::write(lib.join("lib.typ"), body).unwrap();
    dir
}

fn ctx_with<'a>(root: &'a Path, overrides: &'a HashMap<String, PathBuf>) -> ProviderCtx<'a> {
    ProviderCtx { root, overrides }
}

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
fn bytes_provider_serves_registered_bytes_without_copying() {
    let id = root_file_id("data.bin").unwrap();
    let shared = typst::foundations::Bytes::new(b"payload".to_vec());
    let provider = BytesProvider::new().with_bytes(id, shared.clone());
    let overrides = HashMap::new();
    let ctx = ctx_with(Path::new("/virtual"), &overrides);
    let served = provider.read(id, &ctx).unwrap();
    assert_eq!(served.as_ptr(), shared.as_ptr());
    assert_eq!(served.as_slice(), b"payload");
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

/// The path is `tasks\a.typ` under the root on Windows; the id is spelled with
/// `/` on every platform.
#[test]
fn project_file_id_names_a_file_in_a_subfolder_with_slashes() {
    let root = Path::new("/project");
    let id = project_file_id(root, &root.join("tasks").join("a.typ")).unwrap();
    assert!(matches!(id.root(), VirtualRoot::Project));
    assert_eq!(id.vpath().get_with_slash(), "/tasks/a.typ");
}

#[test]
fn project_file_id_refuses_a_file_outside_the_root() {
    let err = project_file_id(Path::new("/project"), Path::new("/elsewhere/a.typ")).unwrap_err();
    assert!(
        err.to_string().contains("is not under project root"),
        "{err}"
    );
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

/// The reader and [`platform_package_root`] are one answer, so a host that
/// installs a package under that root is read back from it.
#[test]
fn resolve_path_local_package_without_override_uses_platform_dir() {
    let id = local_package_file_id("mindtape", "0.2.0", "lib.typ").unwrap();
    let ctx = ProviderCtx {
        root: Path::new("/proj"),
        overrides: &HashMap::new(),
    };
    let expected = platform_package_root()
        .unwrap()
        .join("local")
        .join("mindtape")
        .join("0.2.0")
        .join("lib.typ");
    assert_eq!(resolve_path(id, &ctx).unwrap(), expected);
}

/// A stand-in for `dirs::data_dir()`, so the fall-through is asserted without
/// depending on the machine running the test.
#[allow(clippy::unnecessary_wraps)] // must match `dirs::data_dir`'s signature
fn machine_data() -> Option<PathBuf> {
    Some(PathBuf::from("/machine/data"))
}

/// Unset, the root is the data directory joined with `typst/packages`.
#[test]
fn an_unset_package_path_reads_the_data_dir() {
    assert_eq!(
        package_root_from(None, machine_data),
        Some(PathBuf::from("/machine/data/typst/packages"))
    );
}

/// A set `$TYPST_PACKAGE_PATH` names the root whole, as `--package-path` does.
#[test]
fn a_set_package_path_names_the_root_whole() {
    let root = package_root_from(Some(OsString::from("/pinned")), machine_data);
    assert_eq!(root, Some(PathBuf::from("/pinned")));
}

/// Typst refuses an empty value, so it names no root rather than falling
/// through to the machine's own.
#[test]
fn an_empty_package_path_names_no_root() {
    assert_eq!(package_root_from(Some(OsString::new()), machine_data), None);
}

#[test]
fn no_data_dir_names_no_root() {
    assert_eq!(package_root_from(None, || None), None);
}

/// The entry point reads `$TYPST_PACKAGE_PATH`, then `dirs::data_dir()`, the
/// lookup Typst and tinymist share. `data_local_dir()` differs on Windows.
#[test]
fn the_platform_root_is_typsts_lookup() {
    assert_eq!(
        platform_package_root(),
        package_root_from(std::env::var_os(PACKAGE_PATH_ENV), dirs::data_dir)
    );
}

/// The point of the type: `@local` comes from the directory the caller named,
/// not from the machine's — which the test host may well have populated.
#[test]
fn scoped_provider_reads_a_local_package_from_the_given_root() {
    let packages = package_root("#let hi = 1");
    let overrides = HashMap::new();
    let ctx = ctx_with(Path::new("/proj"), &overrides);
    let provider = ScopedDiskProvider::new(Some(packages.path().to_path_buf()));

    let id = local_package_file_id("greet", "0.1.0", "lib.typ").unwrap();
    let bytes = provider.read(id, &ctx).unwrap();
    assert_eq!(bytes.as_slice(), b"#let hi = 1");
}

/// `None` is an answer, not a fall-through: a host that says its data
/// directory does not exist must not silently read the machine's.
#[test]
fn scoped_provider_without_a_root_misses_every_local_package() {
    let overrides = HashMap::new();
    let ctx = ctx_with(Path::new("/proj"), &overrides);
    let provider = ScopedDiskProvider::new(None);

    let id = local_package_file_id("greet", "0.1.0", "lib.typ").unwrap();
    assert!(matches!(
        provider.read(id, &ctx),
        Err(FileError::NotFound(_))
    ));
}

/// Only the fallback moves. An in-repo override still wins, so pinning the
/// root never overrides a package the caller pinned by hand.
#[test]
fn scoped_provider_keeps_an_explicit_override_winning() {
    let packages = package_root("from the root");
    let pinned = tempfile::tempdir().unwrap();
    std::fs::write(pinned.path().join("lib.typ"), "from the override").unwrap();

    let mut overrides = HashMap::new();
    overrides.insert("greet".to_owned(), pinned.path().to_path_buf());
    let ctx = ctx_with(Path::new("/proj"), &overrides);
    let provider = ScopedDiskProvider::new(Some(packages.path().to_path_buf()));

    let id = local_package_file_id("greet", "0.1.0", "lib.typ").unwrap();
    let bytes = provider.read(id, &ctx).unwrap();
    assert_eq!(bytes.as_slice(), b"from the override");
}

/// Everything that is not a `@local` lookup reads identically under both
/// providers — the package root is the only difference between them.
#[test]
fn scoped_provider_reads_project_files_like_the_disk_provider() {
    let proj = tempfile::tempdir().unwrap();
    std::fs::write(proj.path().join("doc.typ"), "= Hello").unwrap();
    let overrides = HashMap::new();
    let ctx = ctx_with(proj.path(), &overrides);

    let id = root_file_id("doc.typ").unwrap();
    let plain = DiskProvider.read(id, &ctx).unwrap();
    let scoped = ScopedDiskProvider::new(None).read(id, &ctx).unwrap();
    assert_eq!(plain.as_slice(), scoped.as_slice());
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

/// The first provider wins where it has the file; the second answers where it
/// fails, and the first's error stands when both fail.
#[test]
fn fallback_provider_reads_the_second_only_where_the_first_fails() {
    let id = local_package_file_id("greet", "0.1.0", "lib.typ").unwrap();
    let other = local_package_file_id("greet", "0.1.0", "other.typ").unwrap();
    let installed = package_root("#let installed = 1");
    let embedded = BytesProvider::new().with_file(id, *b"#let embedded = 1");
    let overrides = HashMap::new();
    let ctx = ctx_with(Path::new("/virtual"), &overrides);

    let both = FallbackProvider::new(
        ScopedDiskProvider::new(Some(installed.path().to_path_buf())),
        BytesProvider::new().with_file(id, *b"#let embedded = 1"),
    );
    assert_eq!(
        both.read(id, &ctx).unwrap().as_slice(),
        b"#let installed = 1"
    );

    let empty = tempfile::tempdir().unwrap();
    let only_embedded = FallbackProvider::new(
        ScopedDiskProvider::new(Some(empty.path().to_path_buf())),
        embedded,
    );
    assert_eq!(
        only_embedded.read(id, &ctx).unwrap().as_slice(),
        b"#let embedded = 1"
    );
    assert!(
        matches!(only_embedded.read(other, &ctx), Err(FileError::NotFound(path)) if path.starts_with(empty.path()))
    );
}
