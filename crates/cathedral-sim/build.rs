//! Build-time IO only. Runtime cathedral-sim remains pure.
use sha2::{Digest, Sha256};
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};
fn field(hash: &mut Sha256, bytes: &[u8]) {
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
}
fn collect(root: &Path, dir: &Path, out: &mut Vec<PathBuf>) {
    let mut entries: Vec<_> = fs::read_dir(dir)
        .expect("checkpoint build source directory")
        .map(|e| e.expect("checkpoint source entry").path())
        .collect();
    entries.sort();
    for path in entries {
        let name = path.file_name().unwrap().to_string_lossy();
        // Fixture bytes and test-only code are evidence, never the production
        // implementation identity. Inline cfg(test) code is conservatively
        // included with its source file; no textual Rust parser guesses here.
        if name == "tests" || name == "fixtures" || name.starts_with("tests_") || name == "tests.rs"
        {
            continue;
        }
        if path.is_dir() {
            collect(root, &path, out);
        } else if path
            .extension()
            .is_some_and(|ext| ext == "rs" || ext == "toml" || ext == "json" || ext == "j2")
        {
            out.push(path.strip_prefix(root).unwrap().to_path_buf());
        }
    }
}
fn main() {
    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap())
        .join("../..")
        .canonicalize()
        .unwrap();
    let mut paths = vec![
        PathBuf::from("Cargo.toml"),
        PathBuf::from("Cargo.lock"),
        PathBuf::from("assets/world/navigation.bin"),
        PathBuf::from("lore/places/ombreval_buildings.json"),
        PathBuf::from("crates/cathedral-sim/Cargo.toml"),
        PathBuf::from("crates/cathedral-sim/build.rs"),
        PathBuf::from("crates/cathedral-backends/Cargo.toml"),
    ];
    for dir in [
        "src",
        "crates/cathedral-sim/src",
        "crates/cathedral-backends/src",
        "assets/world",
        "assets/prompts",
    ] {
        collect(&root, &root.join(dir), &mut paths);
    }
    paths.sort();
    paths.dedup();
    let mut source = Sha256::new();
    field(&mut source, b"cathedral-complete-production-source-v1");
    for relative in &paths {
        let path = root.join(relative);
        println!("cargo:rerun-if-changed={}", path.display());
        field(&mut source, relative.to_str().unwrap().as_bytes());
        field(
            &mut source,
            &fs::read(path).expect("checkpoint production source"),
        );
    }
    // Track directory membership too: adding/removing a production source must
    // invalidate identity even if it did not exist in an earlier build script.
    for dir in [
        "src",
        "crates/cathedral-sim/src",
        "crates/cathedral-backends/src",
        "assets/world",
        "assets/prompts",
    ] {
        println!("cargo:rerun-if-changed={}", root.join(dir).display());
    }
    let source: [u8; 32] = source.finalize().into();
    let rustc = env::var_os("RUSTC").expect("Cargo rustc");
    let version = Command::new(rustc).arg("-vV").output().expect("rustc -vV");
    assert!(version.status.success(), "cannot bind checkpoint toolchain");
    let mut build = Sha256::new();
    field(&mut build, b"cathedral-complete-build-v1");
    field(&mut build, &source);
    field(&mut build, &version.stdout);
    let mut rows: Vec<_> = env::vars()
        .filter(|(k, _)| k.starts_with("CARGO_CFG_") || k.starts_with("CARGO_FEATURE_"))
        .collect();
    for key in [
        "TARGET",
        "HOST",
        "PROFILE",
        "OPT_LEVEL",
        "DEBUG",
        "CARGO_ENCODED_RUSTFLAGS",
        "RUSTFLAGS",
        "CARGO_BUILD_RUSTFLAGS",
        "LDFLAGS",
        "RUSTC_WRAPPER",
        "RUSTC_WORKSPACE_WRAPPER",
        "CC",
        "CXX",
        "AR",
    ] {
        println!("cargo:rerun-if-env-changed={key}");
        rows.push((
            key.to_string(),
            env::var(key).unwrap_or_else(|_| "<unset>".to_string()),
        ));
    }
    rows.sort();
    for (key, value) in &rows {
        field(&mut build, key.as_bytes());
        field(&mut build, value.as_bytes());
    }
    let build: [u8; 32] = build.finalize().into();
    let text = format!(
        "pub const SOURCE_SHA256: [u8;32] = {source:?};\npub const BUILD_SHA256: [u8;32] = {build:?};\npub const TOOLCHAIN: &str = {:?};\npub const TARGET: &str = {:?};\n",
        String::from_utf8(version.stdout).unwrap(),
        env::var("TARGET").unwrap()
    );
    fs::write(
        PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("checkpoint_build.rs"),
        text,
    )
    .unwrap();
}
