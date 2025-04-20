use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use sha2::{Digest, Sha256};

// Constants
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

// --- Helper: Get active schema version and path (reads config file) ---
// Reads the config file to determine which local schema file should be used for generation.
fn get_active_schema_info_for_build() -> Result<(String, PathBuf), String> {
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

// --- Main Build Logic ---
fn main() {
    // --- 1. Determine Active Schema for Generation ---
    let (active_version, active_schema_path) = match get_active_schema_info_for_build() {
        Ok(info) => info,
        Err(e) => {
            // If config is missing/invalid, we can't know which schema to use.
            // Print a warning but allow the build to continue if types.rs already exists.
            println!("⚠️ Warning: Could not determine active schema from '{}': {}. Type generation might be skipped or use outdated schema.", CONFIG_FILE, e);
            // If src/types.rs exists, maybe the user intends to use the existing one.
            // If it doesn't exist, the build will likely fail later anyway.
            return;
        }
    };

    // --- 2. Setup Rerun Triggers ---
    // Rerun this build script if the config file changes (to switch active version)
    println!("cargo:rerun-if-changed={}", CONFIG_FILE);
    // Rerun if the *currently active* schema file changes
    if active_schema_path.exists() {
         println!("cargo:rerun-if-changed={}", active_schema_path.display());
    } else {
        // If the active schema doesn't exist, print a warning.
        // The build might fail later if the schema is truly needed and missing.
         println!("⚠️ Warning: Active schema file not found: {}", active_schema_path.display());
    }
     // Rerun if the build script itself changes
     println!("cargo:rerun-if-changed=build.rs");

    // --- 3. Read Active Local Schema Content for Hashing ---
    let local_schema_content = match fs::read_to_string(&active_schema_path) {
        Ok(content) => Some(content),
        Err(e) => {
            println!("⚠️ Warning: Failed to read active local schema '{}': {}. Build might fail if types need regeneration.", active_schema_path.display(), e);
            None
        }
    };

    // --- 4. Proceed with Generation based on ACTIVE schema ---
    if let Some(active_content) = local_schema_content {
        let current_hash = calculate_hash(&active_content);
        let out_dir = env::var("OUT_DIR").expect("OUT_DIR environment variable not set");
        let hash_file_name = format!("{}{}_hash.txt", HASH_FILE_NAME_PREFIX, active_version);
        let hash_file_path = Path::new(&out_dir).join(hash_file_name);
        let previous_hash = fs::read_to_string(&hash_file_path).ok();
        let output_path = Path::new(OUTPUT_PATH);
        let output_exists = output_path.exists();

        // Determine if regeneration is needed
        let needs_regeneration = !output_exists || previous_hash.map_or(true, |ph| ph != current_hash);

        if needs_regeneration {
            println!("💡 Active schema (version '{}', path: {}) changed or output missing. Regenerating types...",
                     active_version, active_schema_path.display());

            // Ensure output dir exists (src/ should exist, but be safe)
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
                // Provide a helpful error message if cargo-typify fails
                panic!(
                    "❌ Failed to run 'cargo typify' for schema '{}'. Is cargo-typify installed (`cargo install cargo-typify`) and in your PATH? Exit status: {}",
                    active_schema_path.display(),
                    typify_status
                );
            }

            // Write the new hash for the ACTIVE version to OUT_DIR
            if let Err(e) = fs::write(&hash_file_path, &current_hash) {
                // Warn, don't panic. Generation succeeded, just caching failed.
                eprintln!(
                    "⚠️ Warning: Failed to write new schema hash to '{}': {}",
                    hash_file_path.display(),
                    e
                );
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
