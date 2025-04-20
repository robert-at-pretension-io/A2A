use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use sha2::{Digest, Sha256};

const REMOTE_SCHEMA_URL: &str = "https://raw.githubusercontent.com/google/A2A/refs/heads/main/specification/json/a2a.json";
const SCHEMAS_DIR: &str = "schemas";
const CONFIG_FILE: &str = "a2a_schema.config";
const OUTPUT_PATH: &str = "src/types.rs"; // Output always goes here
const HASH_FILE_NAME_PREFIX: &str = "a2a_schema_"; // e.g., a2a_schema_v1_hash.txt

// --- Helper: Calculate SHA256 hash ---
fn calculate_hash(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    let result = hasher.finalize();
    format!("{:x}", result)
}

// --- Helper: Get active schema version and path ---
fn get_active_schema_info() -> Result<(String, PathBuf), String> {
    let config_content = fs::read_to_string(CONFIG_FILE)
        .map_err(|e| format!("❌ Failed to read config file '{}': {}", CONFIG_FILE, e))?;

    for line in config_content.lines() {
        if let Some(stripped) = line.trim().strip_prefix("active_version") {
            if let Some(version) = stripped.trim().strip_prefix('=') {
                let version_str = version.trim().trim_matches('"').to_string();
                if !version_str.is_empty() {
                    let filename = format!("a2a_schema_{}.json", version_str);
                    let path = Path::new(SCHEMAS_DIR).join(&filename);
                    return Ok((version_str, path));
                }
            }
        }
    }
    Err(format!("❌ Could not find 'active_version = \"...\"' in {}", CONFIG_FILE))
}

// --- Helper: Fetch remote schema ---
fn fetch_remote_schema() -> Result<String, String> {
    println!("ℹ️ Fetching remote schema from {}...", REMOTE_SCHEMA_URL);
    let response = reqwest::blocking::get(REMOTE_SCHEMA_URL)
        .map_err(|e| format!("Network error fetching schema: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("HTTP error fetching schema: {}", response.status()));
    }

    response.text().map_err(|e| format!("Error reading schema response body: {}", e))
}

// --- Helper: Determine next version string (e.g., v1 -> v2) ---
fn get_next_version(current_version: &str) -> Result<String, String> {
    if !current_version.starts_with('v') {
        return Err(format!("Invalid version format: '{}'. Expected 'vN'.", current_version));
    }
    let num_part = &current_version[1..];
    let current_num: u32 = num_part.parse()
        .map_err(|_| format!("Could not parse version number from '{}'", current_version))?;
    Ok(format!("v{}", current_num + 1))
}

// --- Helper: Save schema content (pretty-printed) ---
fn save_schema(path: &Path, content: &str) -> Result<(), String> {
    // Validate and pretty-print JSON before saving
    let json_value: serde_json::Value = serde_json::from_str(content)
        .map_err(|e| format!("Fetched remote content is not valid JSON: {}", e))?;
    let pretty_content = serde_json::to_string_pretty(&json_value)
        .map_err(|e| format!("Failed to pretty-print JSON: {}", e))?;

    // Ensure schemas directory exists
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create schemas directory '{}': {}", parent.display(), e))?;
    }

    fs::write(path, pretty_content)
        .map_err(|e| format!("Failed to write new schema to '{}': {}", path.display(), e))
}

// --- Main Build Logic ---
fn main() {
    // --- 1. Get Active Local Schema Info ---
    // Note: We removed cargo:rerun-if-changed directives to ensure
    // this script runs on every build to check the remote URL.
    let (active_version, active_schema_path) = match get_active_schema_info() {
        Ok(info) => info,
        Err(e) => panic!("{}", e), // Config error is fatal
    };
    println!("ℹ️ Active schema version: {}", active_version);
    println!("ℹ️ Active schema path: {}", active_schema_path.display());


    // --- 3. Fetch Remote Schema ---
    let remote_schema_content = match fetch_remote_schema() {
        Ok(content) => Some(content),
        Err(e) => {
            println!("⚠️ Warning: {}. Proceeding with local build using version '{}'.", e, active_version);
            None
        }
    };

    // --- 4. Read Active Local Schema ---
    let local_schema_content = match fs::read_to_string(&active_schema_path) {
        Ok(content) => Some(content),
        Err(e) => {
            println!("⚠️ Warning: Failed to read active local schema '{}': {}. Build might fail if types need regeneration.", active_schema_path.display(), e);
            None
        }
    };

    // --- 5. Compare and Handle New Version (if possible) ---
    let mut new_version_detected = false;
    let mut new_version_path_str = String::new();
    let mut next_version_str = String::new();

    if let (Some(remote), Some(local)) = (&remote_schema_content, &local_schema_content) {
        // Normalize whitespace/newlines for comparison robustness
        let remote_norm = remote.trim().replace("\r\n", "\n");
        let local_norm = local.trim().replace("\r\n", "\n");

        if remote_norm != local_norm {
            println!("💡 Remote schema differs from active local schema (version '{}').", active_version);
            match get_next_version(&active_version) {
                Ok(next_v) => {
                    next_version_str = next_v;
                    let new_filename = format!("a2a_schema_{}.json", next_version_str);
                    let new_path = Path::new(SCHEMAS_DIR).join(&new_filename);
                    new_version_path_str = new_path.display().to_string();

                    match save_schema(&new_path, remote) {
                        Ok(_) => {
                            println!("✅ Saved new schema version as '{}'.", new_version_path_str);
                            new_version_detected = true;
                        }
                        Err(e) => println!("⚠️ Warning: Failed to save new schema version: {}", e),
                    }
                }
                Err(e) => println!("⚠️ Warning: Could not determine next schema version: {}. Cannot save new remote schema.", e),
            }
        } else {
             println!("✅ Remote schema matches active local schema (version '{}').", active_version);
        }
    }

    // --- 6. Print User Prompt if New Version Detected ---
    if new_version_detected {
        println!("\n======================================================================");
        println!("🚀 A new version of the A2A schema was detected and saved to:");
        println!("   {}", new_version_path_str);
        println!("👉 To use this new version, update '{}' to set:", CONFIG_FILE);
        println!("   active_version = \"{}\"", next_version_str);
        println!("   Then, run 'cargo build' again to regenerate types.");
        println!("   Continuing build with the currently configured version ('{}').", active_version);
        println!("======================================================================\n");
    }

    // --- 7. Proceed with Generation based on ACTIVE schema ---
    if let Some(active_content) = local_schema_content {
        let current_hash = calculate_hash(&active_content);
        let out_dir = env::var("OUT_DIR").expect("OUT_DIR environment variable not set");
        let hash_file_name = format!("{}{}_hash.txt", HASH_FILE_NAME_PREFIX, active_version);
        let hash_file_path = Path::new(&out_dir).join(hash_file_name);
        let previous_hash = fs::read_to_string(&hash_file_path).ok();
        let output_path = Path::new(OUTPUT_PATH);
        let output_exists = output_path.exists();

        let needs_regeneration = !output_exists || previous_hash.map_or(true, |ph| ph != current_hash);

        if needs_regeneration {
            println!("💡 Active schema (version '{}') changed or output missing. Regenerating types...", active_version);

            // Ensure output dir exists
            if let Some(parent) = output_path.parent() {
                if let Err(e) = fs::create_dir_all(parent) {
                     panic!("❌ Failed to create output directory '{}': {}", parent.display(), e);
                }
            }

            // Run cargo typify using the ACTIVE schema path
            let typify_status = Command::new("cargo")
                .args([
                    "typify",
                    "-o",
                    OUTPUT_PATH,
                    &active_schema_path.to_string_lossy(), // Use ACTIVE path
                ])
                .status()
                .expect("Failed to execute cargo-typify command. Is it installed?");

            if !typify_status.success() {
                panic!(
                    "❌ Failed to run 'cargo typify' for schema '{}'. Is cargo-typify installed (`cargo install cargo-typify`) and in your PATH? Exit status: {}",
                    active_schema_path.display(),
                    typify_status
                );
            }

            // Write the new hash for the ACTIVE version
            if let Err(e) = fs::write(&hash_file_path, &current_hash) {
                eprintln!("⚠️ Warning: Failed to write new schema hash to '{}': {}", hash_file_path.display(), e);
            }

            println!("✅ Successfully generated types into '{}' using schema version '{}'", OUTPUT_PATH, active_version);
        } else {
            println!("✅ Schema hash matches for active version '{}' and output exists. Skipping type generation.", active_version);
        }
    } else {
        // Active local schema couldn't be read.
        if !Path::new(OUTPUT_PATH).exists() {
             println!("❌ Error: Active schema '{}' could not be read and output file '{}' does not exist. Cannot proceed.", active_schema_path.display(), OUTPUT_PATH);
             // Depending on project needs, you might panic here.
             // panic!("Cannot build without schema or generated types.");
        } else {
             println!("ℹ️ Skipping type generation as active schema could not be read, but output file exists.");
        }
    }
}
