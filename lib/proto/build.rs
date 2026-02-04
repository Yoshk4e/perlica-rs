use std::path::Path;

fn main() {
    println!("cargo::rustc-check-cfg=cfg(rust_analyzer)");

    let proto_files = ["perlica.proto", "cmd_id.proto"];

    for proto in &proto_files {
        println!("cargo::rerun-if-changed={proto}");
    }

    if proto_files.iter().all(|f| Path::new(f).exists()) {
        prost_build::Config::new()
            .type_attribute(
                ".",
                "#[derive(serde::Serialize, serde::Deserialize)]\n#[serde(rename_all = \"camelCase\")]",
            )
            .message_attribute(".", r#"#[serde(default)]"#)
            .field_attribute("*.type", "#[serde(rename = \"type\")]")
            .out_dir("out/")
            .compile_protos(&proto_files, &["."])
            .expect("Failed to compile proto files");
    }
}
