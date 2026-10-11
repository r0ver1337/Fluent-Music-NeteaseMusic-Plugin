#![allow(non_snake_case)]
use std::path::Path;
fn verify(directory: &Path) {
    for entry in std::fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            verify(&path);
            continue;
        }
        if path.extension().is_none_or(|extension| extension != "rs") {
            continue;
        }
        assert!(
            path.file_stem()
                .unwrap()
                .to_str()
                .unwrap()
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric()),
            "Non-camelCase internal module {}",
            path.display()
        );
        assert!(
            std::fs::read_to_string(&path).unwrap().lines().count() <= 350,
            "Oversized module {}",
            path.display()
        );
    }
}
#[test]
fn modulesAreSmallAndInternalNamesUseCamelCase() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    verify(&root.join("src"));
    verify(&root.join("tests"));
}
