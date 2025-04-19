use jsonschema::{JSONSchema, ValidationError};
use serde_json::Value;
use std::fs;

// Load the JSON Schema
lazy_static::lazy_static! {
    static ref A2A_SCHEMA: JSONSchema = {
        let schema_str = include_str!("../a2a_schema.json");
        let schema_value: Value = serde_json::from_str(schema_str).unwrap();
        JSONSchema::compile(&schema_value).unwrap()
    };
}

pub fn validate_file(file_path: &str) -> Result<(), String> {
    let file_content = fs::read_to_string(file_path)
        .expect("Failed to read file");
    
    let json: Value = serde_json::from_str(&file_content)
        .expect("Invalid JSON");
    
    // Validate the JSON
    let validation_result = A2A_SCHEMA.validate(&json);
    
    match validation_result {
        Ok(_) => {
            println!("✅ Validation passed!");
            Ok(())
        }
        Err(errors) => {
            println!("❌ Validation failed!");
            
            // Collect error messages into a single string
            let error_messages = errors
                .map(|e| format!("  - {}", e))
                .collect::<Vec<_>>()
                .join("\n");
            
            println!("{}", error_messages);
            
            Err(format!("Validation failed with {} errors", error_messages.lines().count()))
        }
    }
}
