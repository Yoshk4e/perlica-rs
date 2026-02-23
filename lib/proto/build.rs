use std::fs;
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

        // Generate NetMessage trait implementations
        generate_net_message_impls();
    }
}

fn generate_net_message_impls() {
    let prost_output = fs::read_to_string("out/_.rs").expect("Failed to read generated prost file");

    let cmd_id_content = fs::read_to_string("cmd_id.proto").expect("Failed to read cmd_id.proto");

    // Extract all struct names from the generated prost file
    let mut struct_names = std::collections::HashSet::new();
    for line in prost_output.lines() {
        if line.trim().starts_with("pub struct ") {
            if let Some(name) = line
                .trim()
                .strip_prefix("pub struct ")
                .and_then(|s| s.split_whitespace().next())
            {
                struct_names.insert(name.to_string());
            }
        }
    }

    let mut output = String::new();
    output.push_str("// Auto-generated NetMessage implementations\n\n");
    output.push_str("pub trait NetMessage: prost::Message {\n");
    output.push_str("    const CMD_ID: i32;\n");
    output.push_str("}\n\n");

    for line in cmd_id_content.lines() {
        let line = line.trim();
        if line.starts_with("//") || line.is_empty() {
            continue;
        }

        if let Some((name, id)) = parse_enum_line(line) {
            // Only generate impl if the struct actually exists
            if struct_names.contains(&name) {
                output.push_str(&format!(
                    "impl NetMessage for {} {{\n    const CMD_ID: i32 = {};\n}}\n\n",
                    name, id
                ));
            }
        }
    }

    fs::write("out/net_message_impls.rs", output).expect("Failed to write net_message_impls.rs");
}

fn parse_enum_line(line: &str) -> Option<(String, i32)> {
    let parts: Vec<&str> = line.split('=').collect();
    if parts.len() != 2 {
        return None;
    }

    let name = parts[0].trim().to_string();
    let id = parts[1].trim().trim_end_matches(';').parse().ok()?;

    Some((name, id))
}
