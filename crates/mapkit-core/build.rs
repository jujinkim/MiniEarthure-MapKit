use sha2::{Digest, Sha256};
use std::{env, fs, path::{Path, PathBuf}};

fn collect(root: &Path, dir: &Path, files: &mut Vec<PathBuf>) {
    println!("cargo:rerun-if-changed={}", dir.display());
    for entry in fs::read_dir(dir).expect("fingerprint source directory") {
        let path = entry.unwrap().path();
        if path.is_dir() {
            if !matches!(path.file_name().and_then(|n| n.to_str()), Some("tests" | "examples" | "target")) {
                collect(root, &path, files);
            }
        } else if matches!(path.extension().and_then(|n| n.to_str()), Some("rs" | "toml" | "lock" | "json")) {
            files.push(path.strip_prefix(root).unwrap().to_owned());
        }
    }
}

fn main() {
    let root = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("../..");
    let mut files = vec![PathBuf::from("Cargo.toml"), PathBuf::from("Cargo.lock")];
    files.extend(["spec/document.schema.json", "spec/manifest.schema.json"].map(PathBuf::from));
    collect(&root, &root.join("crates"), &mut files);
    files.sort();
    let mut hash = Sha256::new();
    for relative in files {
        let path = root.join(&relative);
        println!("cargo:rerun-if-changed={}", path.display());
        let name = relative.to_str().unwrap().replace('\\', "/");
        let source = fs::read_to_string(path).expect("fingerprint UTF-8 input").replace("\r\n", "\n");
        hash.update((name.len() as u64).to_le_bytes());
        hash.update(name.as_bytes());
        hash.update((source.len() as u64).to_le_bytes());
        hash.update(source.as_bytes());
    }
    println!("cargo:rustc-env=MAPKIT_BUILD_FINGERPRINT={:x}", hash.finalize());
}
