use crate::schema_utils;
use jsonschema::{JSONSchema /* ValidationError */}; // Removed unused
use once_cell::sync::Lazy; // Use once_cell::sync::Lazy
use serde_json::Value;
use std::fs; // Import the schema_utils module

// Load the active JSON Schema lazily based on a2a_schema.config
static ACTIVE_A2A_SCHEMA: Lazy<Result<JSONSchema, String>> = Lazy::new(|| {
    // Get the path to the active schema file
    let (_version, schema_path) = schema_utils::get_active_schema_info()
        .map_err(|e| format!("Failed to determine active schema: {}", e))?;

    println!(
        "ℹ️ Validator loading active schema: {}",
        schema_path.display()
    );

    // Read the schema file content
    let schema_str = fs::read_to_string(&schema_path).map_err(|e| {
        format!(
            "Failed to read schema file '{}': {}",
            schema_path.display(),
            e
        )
    })?;

    // Parse the schema content
    let schema_value: Value = serde_json::from_str(&schema_str).map_err(|e| {
        format!(
            "Failed to parse schema JSON from '{}': {}",
            schema_path.display(),
            e
        )
    })?;

    // Compile the schema
    JSONSchema::compile(&schema_value).map_err(|e| {
        format!(
            "Failed to compile schema '{}': {}",
            schema_path.display(),
            e
        )
    })
});

/// Validate a JSON value directly against the lazily loaded active schema
pub fn validate_json(json: &Value) -> Result<(), String> {
    // Access the lazily loaded schema
    match &*ACTIVE_A2A_SCHEMA {
        Ok(schema) => {
            // Validate the JSON
            let validation_result = schema.validate(json);
            match validation_result {
                Ok(_) => Ok(()),
                Err(errors) => {
                    // Collect error messages into a single string
                    let error_messages = errors
                        .map(|e| format!("  - {}", e))
                        .collect::<Vec<_>>()
                        .join("\n");
                    Err(format!(
                        "Validation failed with errors:\n{}",
                        error_messages
                    ))
                }
            }
        }
        Err(e) => {
            // If schema loading failed, return that error
            Err(format!("Schema loading error: {}", e))
        }
    }
}

pub fn validate_file(file_path: &str) -> Result<(), String> {
    let file_content = fs::read_to_string(file_path)
        .map_err(|e| format!("Failed to read file '{}': {}", file_path, e))?;

    let json: Value = serde_json::from_str(&file_content)
        .map_err(|e| format!("Invalid JSON in file '{}': {}", file_path, e))?;

    // Validate the JSON using the updated function
    let result = validate_json(&json);

    match &result {
        Ok(_) => {
            println!("✅ Validation passed for file: {}", file_path);
        }
        Err(error_message) => {
            println!("❌ Validation failed for file: {}", file_path);
            println!("{}", error_message);
        }
    }

    result
}
