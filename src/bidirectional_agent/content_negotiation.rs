// content_negotiation.rs - A2A Protocol Compliant Content Type Negotiation
//
// This file provides a standards-compliant implementation of the A2A content type
// negotiation, handling inputModes and outputModes from agent cards and properly
// negotiating content formats.

use crate::types::{
    Part, TextPart, DataPart, FilePart, AgentCard, AgentSkill, Message,
};
use crate::bidirectional_agent::error::AgentError;
use serde::{Serialize, Deserialize};
use std::collections::{HashMap, HashSet};
use mime::Mime;
use log::{debug, info, warn, error};

/// Represents content types for input/output negotiation
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContentType {
    /// Text content with specific mime type
    Text(String),
    
    /// Structured data with schema
    Data(Option<String>),
    
    /// Binary file with mime type
    File(String),
}

impl ContentType {
    /// Parse content type from string
    pub fn from_str(content_type: &str) -> Result<Self, AgentError> {
        // Parse the content type
        if content_type.starts_with("text/") {
            // Text content
            Ok(ContentType::Text(content_type.to_string()))
        } else if content_type == "application/json" {
            // JSON data
            Ok(ContentType::Data(None))
        } else if content_type.starts_with("application/json+") {
            // JSON data with schema
            let schema = content_type.strip_prefix("application/json+").map(|s| s.to_string());
            Ok(ContentType::Data(schema))
        } else if content_type.starts_with("image/") 
            || content_type.starts_with("audio/")
            || content_type.starts_with("video/")
            || content_type.starts_with("application/octet-stream") {
            // Binary file
            Ok(ContentType::File(content_type.to_string()))
        } else {
            // Unknown content type
            Err(AgentError::InvalidRequest(format!(
                "Unsupported content type: {}", content_type
            )))
        }
    }
    
    /// Get string representation
    pub fn to_string(&self) -> String {
        match self {
            Self::Text(mime) => mime.clone(),
            Self::Data(None) => "application/json".to_string(),
            Self::Data(Some(schema)) => format!("application/json+{}", schema),
            Self::File(mime) => mime.clone(),
        }
    }
    
    /// Check if content type is compatible with another
    pub fn is_compatible_with(&self, other: &ContentType) -> bool {
        match (self, other) {
            // Text types - use media range checking
            (Self::Text(a), Self::Text(b)) => {
                if let (Ok(mime_a), Ok(mime_b)) = (a.parse::<Mime>(), b.parse::<Mime>()) {
                    // Check if mime_a accepts mime_b - simplified impl since no MediaRange
                    mime_a.type_() == mime_b.type_() && 
                    (mime_a.subtype() == mime_b.subtype() || mime_a.subtype() == "*")
                } else {
                    // If parsing fails, fall back to string comparison
                    a == b
                }
            },
            
            // Data types - check schema compatibility
            (Self::Data(a_schema), Self::Data(b_schema)) => {
                match (a_schema, b_schema) {
                    // Both have no schema - compatible
                    (None, None) => true,
                    
                    // One has schema, other doesn't - schema-less accepts schema
                    (None, Some(_)) => true,
                    (Some(_), None) => false, // Schema requires matching schema
                    
                    // Both have schema - must match
                    (Some(a), Some(b)) => a == b,
                }
            },
            
            // File types - use media range checking
            (Self::File(a), Self::File(b)) => {
                if let (Ok(mime_a), Ok(mime_b)) = (a.parse::<Mime>(), b.parse::<Mime>()) {
                    // Check if mime_a accepts mime_b - simplified impl since no MediaRange
                    mime_a.type_() == mime_b.type_() && 
                    (mime_a.subtype() == mime_b.subtype() || mime_a.subtype() == "*")
                } else {
                    // If parsing fails, fall back to string comparison
                    a == b
                }
            },
            
            // Different types - not compatible
            _ => false,
        }
    }
    
    /// Convert a part to this content type if possible
    pub fn convert_part(&self, part: &Part) -> Result<Part, AgentError> {
        match (self, part) {
            // Text to text conversion
            (Self::Text(_), Part::TextPart(text_part)) => {
                // Assume text can be converted between text formats
                // In a real implementation, you might handle specific conversions
                // between text formats (e.g., plain to markdown)
                Ok(Part::TextPart(TextPart {
                    type_: "text".to_string(),
                    text: text_part.text.clone(),
                    metadata: text_part.metadata.clone().map(|mut m| {
                        // Update mime type in metadata if it exists
                        if let Some(mime_type) = m.get_mut("mimeType") {
                            *mime_type = serde_json::Value::String(self.to_string());
                        } else {
                            m.insert(
                                "mimeType".to_string(),
                                serde_json::Value::String(self.to_string()),
                            );
                        }
                        m
                    }),
                }))
            },
            
            // Data to data conversion
            (Self::Data(_), Part::DataPart(data_part)) => {
                // Data remains data, possibly with schema validation
                // In a real implementation, you would validate against schema
                Ok(Part::DataPart(DataPart {
                    type_: "data".to_string(),
                    data: data_part.data.clone(),
                    metadata: data_part.metadata.clone().map(|mut m| {
                        // Update mime type in metadata if it exists
                        if let Some(mime_type) = m.get_mut("mimeType") {
                            *mime_type = serde_json::Value::String(self.to_string());
                        } else {
                            m.insert(
                                "mimeType".to_string(),
                                serde_json::Value::String(self.to_string()),
                            );
                        }
                        m
                    }),
                }))
            },
            
            // File to file conversion
            (Self::File(_), Part::FilePart(file_part)) => {
                // File remains file, possibly with format conversion
                // In a real implementation, you would handle format conversion
                Ok(Part::FilePart(FilePart {
                    type_: "file".to_string(),
                    file: file_part.file.clone(),
                    metadata: file_part.metadata.clone().map(|mut m| {
                        // Update mime type in metadata if it exists
                        if let Some(mime_type) = m.get_mut("mimeType") {
                            *mime_type = serde_json::Value::String(self.to_string());
                        } else {
                            m.insert(
                                "mimeType".to_string(),
                                serde_json::Value::String(self.to_string()),
                            );
                        }
                        m
                    }),
                }))
            },
            
            // Text to data conversion (if text is JSON)
            (Self::Data(_), Part::TextPart(text_part)) => {
                // Try to parse text as JSON
                match serde_json::from_str::<serde_json::Value>(&text_part.text) {
                    Ok(json_value) => {
                        if let serde_json::Value::Object(data) = json_value {
                            Ok(Part::DataPart(DataPart {
                                type_: "data".to_string(),
                                data,
                                metadata: Some({
                                    let mut metadata = text_part.metadata.clone().unwrap_or_default();
                                    metadata.insert(
                                        "mimeType".to_string(),
                                        serde_json::Value::String(self.to_string()),
                                    );
                                    metadata
                                }),
                            }))
                        } else {
                            // Not an object, return error
                            Err(AgentError::ContentTypeError(format!(
                                "Cannot convert text to data: not a JSON object"
                            )))
                        }
                    },
                    Err(_) => {
                        // Not valid JSON, return error
                        Err(AgentError::ContentTypeError(format!(
                            "Cannot convert text to data: not valid JSON"
                        )))
                    }
                }
            },
            
            // Data to text conversion
            (Self::Text(_), Part::DataPart(data_part)) => {
                // Convert data to JSON string
                let json_str = serde_json::to_string_pretty(&data_part.data)
                    .map_err(|e| AgentError::ContentTypeError(format!(
                        "Failed to serialize data to JSON: {}", e
                    )))?;
                
                Ok(Part::TextPart(TextPart {
                    type_: "text".to_string(),
                    text: json_str,
                    metadata: Some({
                        let mut metadata = data_part.metadata.clone().unwrap_or_default();
                        metadata.insert(
                            "mimeType".to_string(),
                            serde_json::Value::String(self.to_string()),
                        );
                        metadata
                    }),
                }))
            },
            
            // Other conversions not supported
            _ => Err(AgentError::ContentTypeError(format!(
                "Cannot convert {:?} to {:?}", part, self
            ))),
        }
    }
}

/// Manages content type negotiation
pub struct ContentNegotiator {
    /// Agent card with supported content types
    agent_card: AgentCard,
    
    /// Content type converters (for custom conversions)
    converters: HashMap<(String, String), Box<dyn Fn(&Part) -> Result<Part, AgentError> + Send + Sync>>,
}

impl ContentNegotiator {
    /// Create a new content negotiator from an agent card
    pub fn new(agent_card: AgentCard) -> Self {
        Self {
            agent_card,
            converters: HashMap::new(),
        }
    }
    
    /// Get supported input content types
    pub fn supported_input_types(&self) -> Vec<String> {
        self.agent_card.default_input_modes.clone()
    }
    
    /// Get supported output content types
    pub fn supported_output_types(&self) -> Vec<String> {
        self.agent_card.default_output_modes.clone()
    }
    
    /// Register a custom converter for content types
    pub fn register_converter<F>(&mut self, from_type: &str, to_type: &str, converter: F)
    where
        F: Fn(&Part) -> Result<Part, AgentError> + Send + Sync + 'static,
    {
        self.converters.insert(
            (from_type.to_string(), to_type.to_string()),
            Box::new(converter),
        );
    }
    
    /// Find the best matching content type
    pub fn find_best_match(
        &self,
        requested_types: &[String],
        supported_types: &[String],
    ) -> Option<String> {
        // First, try to find an exact match
        for req_type in requested_types {
            if supported_types.contains(req_type) {
                return Some(req_type.clone());
            }
        }
        
        // If no exact match, parse and try compatibility matching
        for req_type_str in requested_types {
            if let Ok(req_type) = ContentType::from_str(req_type_str) {
                for sup_type_str in supported_types {
                    if let Ok(sup_type) = ContentType::from_str(sup_type_str) {
                        if req_type.is_compatible_with(&sup_type) {
                            return Some(sup_type_str.clone());
                        }
                    }
                }
            }
        }
        
        // No match found, use the first supported type as default
        supported_types.first().cloned()
    }
    
    /// Negotiate input content type
    pub fn negotiate_input(
        &self,
        requested_types: &[String],
    ) -> Result<String, AgentError> {
        let supported = self.supported_input_types();
        self.find_best_match(requested_types, &supported)
            .ok_or_else(|| AgentError::ContentTypeError(format!(
                "No compatible input content type found. Requested: {:?}, Supported: {:?}",
                requested_types, supported
            )))
    }
    
    /// Negotiate output content type
    pub fn negotiate_output(
        &self,
        requested_types: &[String],
    ) -> Result<String, AgentError> {
        let supported = self.supported_output_types();
        self.find_best_match(requested_types, &supported)
            .ok_or_else(|| AgentError::ContentTypeError(format!(
                "No compatible output content type found. Requested: {:?}, Supported: {:?}",
                requested_types, supported
            )))
    }
    
    /// Convert a message to the requested content types
    pub fn convert_message(
        &self,
        message: &Message,
        output_types: &[String],
    ) -> Result<Message, AgentError> {
        let mut converted_parts = Vec::new();
        
        // Process each part
        for part in &message.parts {
            // Determine current content type
            let current_type = match part {
                Part::TextPart(_) => ContentType::Text("text/plain".to_string()),
                Part::DataPart(_) => ContentType::Data(None),
                Part::FilePart(file_part) => {
                    let mime = file_part.file.mime_type.clone()
                        .unwrap_or_else(|| "application/octet-stream".to_string());
                    ContentType::File(mime)
                },
            };
            
            // Find best output type for this part
            let target_type_str = self.find_best_match(output_types, &self.supported_output_types())
                .ok_or_else(|| AgentError::ContentTypeError(
                    "No compatible output type found".to_string()
                ))?;
            
            let target_type = ContentType::from_str(&target_type_str)?;
            
            // Check if conversion is needed
            if let Ok(current_type_str) = serde_json::to_string(&current_type) {
                if current_type_str == target_type_str {
                    // No conversion needed
                    converted_parts.push(part.clone());
                    continue;
                }
            }
            
            // Check for custom converter
            let converter_key = (current_type.to_string(), target_type.to_string());
            if let Some(converter) = self.converters.get(&converter_key) {
                // Use custom converter
                let converted = converter(part)?;
                converted_parts.push(converted);
            } else {
                // Use built-in conversion
                let converted = target_type.convert_part(part)?;
                converted_parts.push(converted);
            }
        }
        
        // Create converted message
        Ok(Message {
            role: message.role.clone(),
            parts: converted_parts,
            metadata: message.metadata.clone(),
        })
    }
    
    /// Get supported content types for a specific skill
    pub fn get_skill_content_types(
        &self,
        skill_id: &str,
    ) -> (Vec<String>, Vec<String>) {
        // Look up the skill
        if let Some(skill) = self.agent_card.skills.iter().find(|s| s.id == skill_id) {
            // Use skill-specific types if available, otherwise default to agent defaults
            let input_modes = skill.input_modes.clone()
                .unwrap_or_else(|| self.agent_card.default_input_modes.clone());
            
            let output_modes = skill.output_modes.clone()
                .unwrap_or_else(|| self.agent_card.default_output_modes.clone());
            
            (input_modes, output_modes)
        } else {
            // Skill not found, use agent defaults
            (
                self.agent_card.default_input_modes.clone(),
                self.agent_card.default_output_modes.clone(),
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    
    // Helper to create a test agent card
    fn create_test_agent_card() -> AgentCard {
        AgentCard {
            name: "Test Agent".to_string(),
            version: "1.0".to_string(),
            url: "https://example.com/agent".to_string(),
            capabilities: crate::types::AgentCapabilities {
                streaming: true,
                push_notifications: true,
                state_transition_history: true,
            },
            skills: vec![
                AgentSkill {
                    id: "general".to_string(),
                    name: "General".to_string(),
                    description: Some("General purpose skill".to_string()),
                    tags: Some(vec!["general".to_string()]),
                    examples: None,
                    input_modes: None, // Use agent defaults
                    output_modes: None, // Use agent defaults
                },
                AgentSkill {
                    id: "code".to_string(),
                    name: "Code".to_string(),
                    description: Some("Code generation skill".to_string()),
                    tags: Some(vec!["code".to_string()]),
                    examples: None,
                    input_modes: Some(vec![
                        "text/plain".to_string(),
                        "application/json".to_string(),
                    ]),
                    output_modes: Some(vec![
                        "text/plain".to_string(),
                        "text/markdown".to_string(),
                    ]),
                },
            ],
            description: Some("Test agent for content negotiation".to_string()),
            documentation_url: None,
            authentication: None,
            default_input_modes: vec![
                "text/plain".to_string(),
                "text/markdown".to_string(),
                "application/json".to_string(),
                "image/png".to_string(),
            ],
            default_output_modes: vec![
                "text/plain".to_string(),
                "text/markdown".to_string(),
                "application/json".to_string(),
            ],
            provider: None,
        }
    }
    
    #[test]
    fn test_content_type_parsing() {
        // Test text content types
        let text_plain = ContentType::from_str("text/plain").unwrap();
        assert!(matches!(text_plain, ContentType::Text(mime) if mime == "text/plain"));
        
        let text_markdown = ContentType::from_str("text/markdown").unwrap();
        assert!(matches!(text_markdown, ContentType::Text(mime) if mime == "text/markdown"));
        
        // Test data content types
        let json = ContentType::from_str("application/json").unwrap();
        assert!(matches!(json, ContentType::Data(None)));
        
        let json_schema = ContentType::from_str("application/json+schema1").unwrap();
        assert!(matches!(json_schema, ContentType::Data(Some(schema)) if schema == "schema1"));
        
        // Test file content types
        let png = ContentType::from_str("image/png").unwrap();
        assert!(matches!(png, ContentType::File(mime) if mime == "image/png"));
        
        let binary = ContentType::from_str("application/octet-stream").unwrap();
        assert!(matches!(binary, ContentType::File(mime) if mime == "application/octet-stream"));
        
        // Test invalid content type
        let invalid = ContentType::from_str("invalid/type");
        assert!(invalid.is_err());
    }
    
    #[test]
    fn test_content_type_compatibility() {
        // Text compatibility
        let text_plain = ContentType::from_str("text/plain").unwrap();
        let text_markdown = ContentType::from_str("text/markdown").unwrap();
        let text_any = ContentType::from_str("text/*").unwrap();
        
        assert!(text_plain.is_compatible_with(&text_plain));
        assert!(!text_plain.is_compatible_with(&text_markdown));
        assert!(text_any.is_compatible_with(&text_plain));
        assert!(text_any.is_compatible_with(&text_markdown));
        
        // Data compatibility
        let json = ContentType::from_str("application/json").unwrap();
        let json_schema1 = ContentType::from_str("application/json+schema1").unwrap();
        let json_schema2 = ContentType::from_str("application/json+schema2").unwrap();
        
        assert!(json.is_compatible_with(&json));
        assert!(json.is_compatible_with(&json_schema1));
        assert!(json_schema1.is_compatible_with(&json_schema1));
        assert!(!json_schema1.is_compatible_with(&json));
        assert!(!json_schema1.is_compatible_with(&json_schema2));
        
        // File compatibility
        let png = ContentType::from_str("image/png").unwrap();
        let jpeg = ContentType::from_str("image/jpeg").unwrap();
        let image_any = ContentType::from_str("image/*").unwrap();
        
        assert!(png.is_compatible_with(&png));
        assert!(!png.is_compatible_with(&jpeg));
        assert!(image_any.is_compatible_with(&png));
        assert!(image_any.is_compatible_with(&jpeg));
        
        // Cross-type compatibility
        assert!(!text_plain.is_compatible_with(&json));
        assert!(!json.is_compatible_with(&png));
        assert!(!png.is_compatible_with(&text_plain));
    }
    
    #[test]
    fn test_part_conversion() {
        // Text to text conversion
        let text_plain = ContentType::from_str("text/plain").unwrap();
        let text_part = Part::TextPart(TextPart {
            type_: "text".to_string(),
            text: "Hello, world!".to_string(),
            metadata: None,
        });
        
        let converted = text_plain.convert_part(&text_part).unwrap();
        assert!(matches!(converted, Part::TextPart(_)));
        
        // Data to text conversion
        let json_type = ContentType::from_str("application/json").unwrap();
        let data_part = Part::DataPart(DataPart {
            type_: "data".to_string(),
            data: {
                let mut map = serde_json::Map::new();
                map.insert("hello".to_string(), json!("world"));
                map
            },
            metadata: None,
        });
        
        let converted = text_plain.convert_part(&data_part).unwrap();
        assert!(matches!(converted, Part::TextPart(_)));
        if let Part::TextPart(text) = converted {
            assert!(text.text.contains("hello"));
            assert!(text.text.contains("world"));
        }
        
        // Text to data conversion (JSON text)
        let json_text = Part::TextPart(TextPart {
            type_: "text".to_string(),
            text: r#"{"hello":"world"}"#.to_string(),
            metadata: None,
        });
        
        let converted = json_type.convert_part(&json_text).unwrap();
        assert!(matches!(converted, Part::DataPart(_)));
        if let Part::DataPart(data) = converted {
            assert_eq!(data.data.get("hello").unwrap().as_str().unwrap(), "world");
        }
        
        // Text to data conversion (non-JSON text) - should fail
        let plain_text = Part::TextPart(TextPart {
            type_: "text".to_string(),
            text: "Not JSON".to_string(),
            metadata: None,
        });
        
        let result = json_type.convert_part(&plain_text);
        assert!(result.is_err());
    }
    
    #[test]
    fn test_negotiator_input_types() {
        let card = create_test_agent_card();
        let negotiator = ContentNegotiator::new(card);
        
        // Check supported types
        let input_types = negotiator.supported_input_types();
        assert_eq!(input_types.len(), 4);
        assert!(input_types.contains(&"text/plain".to_string()));
        assert!(input_types.contains(&"text/markdown".to_string()));
        assert!(input_types.contains(&"application/json".to_string()));
        assert!(input_types.contains(&"image/png".to_string()));
    }
    
    #[test]
    fn test_negotiator_best_match() {
        let card = create_test_agent_card();
        let negotiator = ContentNegotiator::new(card);
        
        // Exact match
        let requested = vec!["text/plain".to_string()];
        let supported = vec![
            "text/plain".to_string(),
            "text/markdown".to_string(),
        ];
        let best = negotiator.find_best_match(&requested, &supported);
        assert_eq!(best, Some("text/plain".to_string()));
        
        // Compatible match
        let requested = vec!["text/*".to_string()];
        let supported = vec![
            "text/plain".to_string(),
            "text/markdown".to_string(),
        ];
        let best = negotiator.find_best_match(&requested, &supported);
        assert_eq!(best, Some("text/plain".to_string()));
        
        // No match
        let requested = vec!["image/png".to_string()];
        let supported = vec![
            "text/plain".to_string(),
            "text/markdown".to_string(),
        ];
        let best = negotiator.find_best_match(&requested, &supported);
        assert_eq!(best, Some("text/plain".to_string())); // Default to first supported
    }
    
    #[test]
    fn test_negotiator_convert_message() {
        let card = create_test_agent_card();
        let negotiator = ContentNegotiator::new(card);
        
        // Create a message with text and data parts
        let message = Message {
            role: crate::types::Role::Agent,
            parts: vec![
                Part::TextPart(TextPart {
                    type_: "text".to_string(),
                    text: "Hello, world!".to_string(),
                    metadata: None,
                }),
                Part::DataPart(DataPart {
                    type_: "data".to_string(),
                    data: {
                        let mut map = serde_json::Map::new();
                        map.insert("hello".to_string(), json!("world"));
                        map
                    },
                    metadata: None,
                }),
            ],
            metadata: None,
        };
        
        // Convert to text/plain
        let output_types = vec!["text/plain".to_string()];
        let converted = negotiator.convert_message(&message, &output_types).unwrap();
        
        // Both parts should be TextPart
        assert_eq!(converted.parts.len(), 2);
        assert!(matches!(converted.parts[0], Part::TextPart(_)));
        assert!(matches!(converted.parts[1], Part::TextPart(_)));
        
        if let Part::TextPart(text) = &converted.parts[1] {
            // Data part should have been converted to text
            assert!(text.text.contains("hello"));
            assert!(text.text.contains("world"));
        }
        
        // Convert to application/json
        let output_types = vec!["application/json".to_string()];
        let result = negotiator.convert_message(&message, &output_types);
        
        // This should fail for the text part (non-JSON text)
        assert!(result.is_err());
    }
    
    #[test]
    fn test_skill_content_types() {
        let card = create_test_agent_card();
        let negotiator = ContentNegotiator::new(card);
        
        // Check general skill (uses agent defaults)
        let (input, output) = negotiator.get_skill_content_types("general");
        assert_eq!(input.len(), 4); // Agent defaults
        assert_eq!(output.len(), 3); // Agent defaults
        
        // Check code skill (uses skill-specific types)
        let (input, output) = negotiator.get_skill_content_types("code");
        assert_eq!(input.len(), 2); // Skill-specific
        assert!(input.contains(&"text/plain".to_string()));
        assert!(input.contains(&"application/json".to_string()));
        
        assert_eq!(output.len(), 2); // Skill-specific
        assert!(output.contains(&"text/plain".to_string()));
        assert!(output.contains(&"text/markdown".to_string()));
        
        // Check non-existent skill (uses agent defaults)
        let (input, output) = negotiator.get_skill_content_types("nonexistent");
        assert_eq!(input.len(), 4); // Agent defaults
        assert_eq!(output.len(), 3); // Agent defaults
    }
}