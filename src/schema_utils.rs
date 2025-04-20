use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

// Constants needed by helper functions
const REMOTE_SCHEMA_URL: &str = "https://raw.githubusercontent.com/google/A2A/refs/heads/main/specification/json/a2a.json";
const SCHEMAS_DIR: &str = "schemas";
const CONFIG_FILE: &str = "a2a_schema.config";

#[derive(Debug, PartialEq)]
pub enum SchemaCheckResult {
    NoChange,
    NewVersionSaved(String), // Contains the path to the new file
}

// --- Helper: Get active schema version and path ---
// (Keep this function private to the module or make it pub if needed elsewhere)
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
// (Keep this function private to the module)
fn fetch_remote_schema() -> Result<String, String> {
    println!("ℹ️ Fetching remote schema from {}...", REMOTE_SCHEMA_URL);
    let response = reqwest::blocking::get(REMOTE_SCHEMA_URL)
         // Use map_err for reqwest errors
        .map_err(|e| format!("Network error fetching schema: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("HTTP error fetching schema: {}", response.status()));
    }

    response.text().map_err(|e| format!("Error reading schema response body: {}", e))
}


// --- Helper: Determine next version string (e.g., v1 -> v2) ---
// (Keep this function private to the module)
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
// (Keep this function private to the module)
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

/// Fetches the remote schema, compares it with the active local one,
/// and saves a new version if they differ.
pub fn check_and_download_remote_schema() -> Result<SchemaCheckResult, String> {
    // --- 1. Get Active Local Schema Info ---
    let (active_version, active_schema_path) = get_active_schema_info()?;
    println!("ℹ️ Active schema version: {}", active_version);
    println!("ℹ️ Active schema path: {}", active_schema_path.display());

    // --- 2. Fetch Remote Schema ---
    let remote_schema_content = fetch_remote_schema()?;

    // --- 3. Read Active Local Schema ---
    let local_schema_content = match fs::read_to_string(&active_schema_path) {
        Ok(content) => content,
        Err(e) => {
            // If the active local schema doesn't exist, we should probably save the fetched one
            println!("⚠️ Warning: Failed to read active local schema '{}': {}. Will attempt to save fetched schema.", active_schema_path.display(), e);
            // Treat this as a difference to force saving
            String::new() // Empty string will ensure difference check passes
        }
    };

    // --- 4. Compare and Handle New Version ---
    // Normalize whitespace/newlines for comparison robustness
    let remote_norm = remote_schema_content.trim().replace("\r\n", "\n");
    let local_norm = local_schema_content.trim().replace("\r\n", "\n");

    if remote_norm != local_norm {
        println!("💡 Remote schema differs from active local schema (version '{}').", active_version);
        let next_version_str = get_next_version(&active_version)?;
        let new_filename = format!("a2a_schema_{}.json", next_version_str);
        let new_path = Path::new(SCHEMAS_DIR).join(&new_filename);
        let new_version_path_str = new_path.display().to_string();

        save_schema(&new_path, &remote_schema_content)?;
        println!("✅ Saved new schema version as '{}'.", new_version_path_str);
        Ok(SchemaCheckResult::NewVersionSaved(new_version_path_str))
    } else {
        println!("✅ Remote schema matches active local schema (version '{}').", active_version);
        Ok(SchemaCheckResult::NoChange)
    }
}
