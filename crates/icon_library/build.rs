use std::env;
use std::fmt::Write as _;
use std::fs;
use std::path::Path;

fn main() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    let root_dir = Path::new(&manifest_dir).join("../..");
    let examples_dir = root_dir.join("resources/examples");
    let kit_dir = root_dir.join("resources/kit_icons");

    println!("cargo:rerun-if-changed=../../resources/examples");
    println!("cargo:rerun-if-changed=../../resources/kit_icons");

    let mut out = String::from("// Auto-generated bundled icon catalog\n");
    out.push_str("pub static BUNDLED_ICONS: &[BundledIconEntry] = &[\n");

    // 1. Add built-in animated GPA examples first
    if let Ok(entries) = fs::read_dir(&examples_dir) {
        let mut files: Vec<_> = entries.flatten().collect();
        files.sort_by_key(std::fs::DirEntry::file_name);
        for entry in files {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("gpa") {
                let file_name = path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or_default();
                let stem = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or_default();
                let _ = writeln!(
                    out,
                    "    BundledIconEntry {{ name: {stem:?}, file_name: {file_name:?}, category: IconCategory::GpaExample, xml: include_str!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/../../resources/examples/{file_name}\")) }},"
                );
            }
        }
    }

    // 2. Add all Icon Development Kit icons (.gpa and .svg)
    if let Ok(entries) = fs::read_dir(&kit_dir) {
        let mut files: Vec<_> = entries.flatten().collect();
        files.sort_by_key(std::fs::DirEntry::file_name);
        for entry in files {
            let path = entry.path();
            let ext = path.extension().and_then(|s| s.to_str()).unwrap_or_default();
            if ext == "svg" || ext == "gpa" {
                let file_name = path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or_default();
                let stem = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or_default();
                let category = if ext == "gpa" {
                    "IconCategory::GpaExample"
                } else {
                    "IconCategory::DevKit"
                };
                let _ = writeln!(
                    out,
                    "    BundledIconEntry {{ name: {stem:?}, file_name: {file_name:?}, category: {category}, xml: include_str!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/../../resources/kit_icons/{file_name}\")) }},"
                );
            }
        }
    }

    out.push_str("];\n");

    let out_dir = env::var("OUT_DIR").expect("OUT_DIR");
    fs::write(Path::new(&out_dir).join("bundled_icons.rs"), out)
        .expect("Failed to write bundled_icons.rs");
}
