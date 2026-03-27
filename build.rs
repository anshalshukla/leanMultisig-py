use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();

    // Find rec_aggregation's CARGO_MANIFEST_DIR via cargo metadata
    let output = Command::new(env::var("CARGO").unwrap_or_else(|_| "cargo".to_string()))
        .args(["metadata", "--format-version=1"])
        .output()
        .expect("Failed to run cargo metadata");

    let metadata: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("Failed to parse cargo metadata");

    let rec_agg_dir = metadata["packages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["name"].as_str() == Some("rec_aggregation"))
        .map(|p| {
            PathBuf::from(p["manifest_path"].as_str().unwrap())
                .parent()
                .unwrap()
                .to_path_buf()
        })
        .expect("rec_aggregation package not found in workspace");

    // Collect all .py files from rec_aggregation
    let py_files: Vec<(String, String)> = fs::read_dir(&rec_agg_dir)
        .expect("Failed to read rec_aggregation directory")
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("py") {
                let name = path.file_name()?.to_str()?.to_string();
                let content = fs::read_to_string(&path).ok()?;
                Some((name, content))
            } else {
                None
            }
        })
        .collect();

    // Generate embedded_py.rs
    let mut code = String::new();
    code.push_str("/// Auto-generated: embedded .py files from rec_aggregation.\n");
    code.push_str("/// These are written to disk at runtime so the lean compiler can read them.\n\n");
    code.push_str(&format!(
        "pub const REC_AGGREGATION_MANIFEST_DIR: &str = {:?};\n\n",
        rec_agg_dir.to_str().unwrap()
    ));
    code.push_str(&format!(
        "pub const EMBEDDED_PY_FILES: &[(&str, &str)] = &[\n"
    ));
    for (name, content) in &py_files {
        code.push_str(&format!("    ({:?}, {:?}),\n", name, content));
    }
    code.push_str("];\n");

    let dest = Path::new(&out_dir).join("embedded_py.rs");
    fs::write(&dest, code).expect("Failed to write embedded_py.rs");

    // Rerun if any .py files change
    println!("cargo:rerun-if-changed=build.rs");
    for (name, _) in &py_files {
        println!(
            "cargo:rerun-if-changed={}",
            rec_agg_dir.join(name).display()
        );
    }
}
