use std::path::Path;

fn main() {
    let resources_dir = Path::new("../../resources");

    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<gresources>
  <gresource prefix="/dev/boxxy/GpaIconsAnimator">
    <file compressed="true">style.css</file>
  </gresource>
</gresources>
"#;

    let xml_path = resources_dir.join("resources.gresource.xml");
    let existing = std::fs::read_to_string(&xml_path).unwrap_or_default();
    if existing != xml {
        std::fs::write(&xml_path, xml).expect("Failed to write resources.gresource.xml");
    }

    println!("cargo:rerun-if-changed=../../resources/style.css");

    glib_build_tools::compile_resources(
        &["../../resources"],
        "../../resources/resources.gresource.xml",
        "compiled.gresource",
    );
}
