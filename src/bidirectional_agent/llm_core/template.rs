//! Prompt template system.
//!
//! This module provides a flexible system for managing and rendering prompt templates
//! from files, with variable substitution.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::RwLock;
use anyhow::{Result, Context, anyhow};
use lazy_static::lazy_static;

lazy_static! {
    /// Global templates cache to avoid repeated filesystem access
    static ref TEMPLATE_CACHE: RwLock<HashMap<String, String>> = RwLock::new(HashMap::new());
}

/// Template manager for loading and rendering prompt templates
pub struct TemplateManager {
    /// Base directory for template files
    template_dir: PathBuf,
    
    /// Whether to use caching (default: true)
    use_cache: bool,
}

impl TemplateManager {
    /// Create a new template manager with the given base directory
    pub fn new<P: AsRef<Path>>(template_dir: P) -> Self {
        Self {
            template_dir: template_dir.as_ref().to_path_buf(),
            use_cache: true,
        }
    }
    
    /// Create a new template manager with the default template directory
    pub fn with_default_dir() -> Self {
        // Default template directory is src/bidirectional_agent/llm_core/prompts
        let default_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("bidirectional_agent")
            .join("llm_core")
            .join("prompts");
        
        Self::new(default_dir)
    }
    
    /// Disable caching for this template manager (useful for development/testing)
    pub fn disable_cache(mut self) -> Self {
        self.use_cache = false;
        self
    }
    
    /// Load a template from a file
    pub fn load_template(&self, template_name: &str) -> Result<String> {
        // Check cache first if enabled
        if self.use_cache {
            let cache = TEMPLATE_CACHE.read().unwrap();
            if let Some(template) = cache.get(template_name) {
                return Ok(template.clone());
            }
        }
        
        // Construct path with .txt extension if not provided
        let template_path = if template_name.ends_with(".txt") {
            self.template_dir.join(template_name)
        } else {
            self.template_dir.join(format!("{}.txt", template_name))
        };
        
        // Load template from file
        let template = fs::read_to_string(&template_path)
            .with_context(|| format!("Failed to read template file: {:?}", template_path))?;
        
        // Cache if enabled
        if self.use_cache {
            let mut cache = TEMPLATE_CACHE.write().unwrap();
            cache.insert(template_name.to_string(), template.clone());
        }
        
        Ok(template)
    }
    
    /// Render a template with the given variables
    pub fn render(&self, template_name: &str, variables: &HashMap<String, String>) -> Result<String> {
        // Load template
        let template = self.load_template(template_name)?;
        
        // Render template by replacing variables
        let mut rendered = template.clone();
        
        for (name, value) in variables {
            let placeholder = format!("{{{{{}}}}}", name);
            rendered = rendered.replace(&placeholder, value);
        }
        
        // Check if any placeholders remain
        if rendered.contains("{{") && rendered.contains("}}") {
            // Try to find the first remaining placeholder for a helpful error message
            let start = rendered.find("{{").unwrap();
            let end = rendered[start..].find("}}").unwrap() + 2;
            let placeholder = &rendered[start..start + end];
            
            return Err(anyhow!("Unfilled placeholder in template: {}", placeholder));
        }
        
        Ok(rendered)
    }
    
    /// Get a list of all available template names
    pub fn list_templates(&self) -> Result<Vec<String>> {
        let entries = fs::read_dir(&self.template_dir)
            .with_context(|| format!("Failed to read template directory: {:?}", self.template_dir))?;
        
        let mut templates = Vec::new();
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            
            if path.is_file() && path.extension().map_or(false, |ext| ext == "txt") {
                if let Some(filename) = path.file_stem() {
                    if let Some(name) = filename.to_str() {
                        templates.push(name.to_string());
                    }
                }
            }
        }
        
        Ok(templates)
    }
    
    /// Clear the template cache
    pub fn clear_cache() {
        let mut cache = TEMPLATE_CACHE.write().unwrap();
        cache.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;
    
    #[test]
    fn test_template_loading() {
        // Create a temporary directory for test templates
        let temp_dir = tempdir().unwrap();
        let template_path = temp_dir.path().join("test_template.txt");
        
        // Create a test template file
        let mut file = File::create(&template_path).unwrap();
        file.write_all(b"Hello, {{name}}!").unwrap();
        
        // Create template manager
        let manager = TemplateManager::new(temp_dir.path());
        
        // Load template
        let template = manager.load_template("test_template").unwrap();
        assert_eq!(template, "Hello, {{name}}!");
    }
    
    #[test]
    fn test_template_rendering() {
        // Create a temporary directory for test templates
        let temp_dir = tempdir().unwrap();
        let template_path = temp_dir.path().join("greeting.txt");
        
        // Create a test template file
        let mut file = File::create(&template_path).unwrap();
        file.write_all(b"Hello, {{name}}! Welcome to {{place}}.").unwrap();
        
        // Create template manager
        let manager = TemplateManager::new(temp_dir.path());
        
        // Create variables
        let mut variables = HashMap::new();
        variables.insert("name".to_string(), "John".to_string());
        variables.insert("place".to_string(), "Wonderland".to_string());
        
        // Render template
        let rendered = manager.render("greeting", &variables).unwrap();
        assert_eq!(rendered, "Hello, John! Welcome to Wonderland.");
    }
    
    #[test]
    fn test_missing_variable() {
        // Create a temporary directory for test templates
        let temp_dir = tempdir().unwrap();
        let template_path = temp_dir.path().join("missing.txt");
        
        // Create a test template file
        let mut file = File::create(&template_path).unwrap();
        file.write_all(b"Hello, {{name}}! Today is {{date}}.").unwrap();
        
        // Create template manager
        let manager = TemplateManager::new(temp_dir.path());
        
        // Create variables with missing "date"
        let mut variables = HashMap::new();
        variables.insert("name".to_string(), "John".to_string());
        
        // Render template should fail
        let result = manager.render("missing", &variables);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("{{date}}"));
    }
    
    #[test]
    fn test_list_templates() {
        // Create a temporary directory for test templates
        let temp_dir = tempdir().unwrap();
        
        // Create several test template files
        File::create(temp_dir.path().join("template1.txt")).unwrap();
        File::create(temp_dir.path().join("template2.txt")).unwrap();
        File::create(temp_dir.path().join("template3.txt")).unwrap();
        
        // Create a non-template file
        File::create(temp_dir.path().join("not_a_template.md")).unwrap();
        
        // Create template manager
        let manager = TemplateManager::new(temp_dir.path());
        
        // List templates
        let templates = manager.list_templates().unwrap();
        
        // Should find 3 templates
        assert_eq!(templates.len(), 3);
        assert!(templates.contains(&"template1".to_string()));
        assert!(templates.contains(&"template2".to_string()));
        assert!(templates.contains(&"template3".to_string()));
        
        // Should not contain non-template file
        assert!(!templates.contains(&"not_a_template".to_string()));
    }
    
    #[test]
    fn test_caching() {
        // Create a temporary directory for test templates
        let temp_dir = tempdir().unwrap();
        let template_path = temp_dir.path().join("cached.txt");
        
        // Create a test template file
        let mut file = File::create(&template_path).unwrap();
        file.write_all(b"Original content").unwrap();
        
        // Create template manager with caching
        let manager = TemplateManager::new(temp_dir.path());
        
        // Load template (will be cached)
        let template1 = manager.load_template("cached").unwrap();
        assert_eq!(template1, "Original content");
        
        // Modify the file
        let mut file = File::create(&template_path).unwrap();
        file.write_all(b"Modified content").unwrap();
        
        // Load template again (should get cached version)
        let template2 = manager.load_template("cached").unwrap();
        assert_eq!(template2, "Original content");
        
        // Clear cache
        TemplateManager::clear_cache();
        
        // Load template again (should get updated content)
        let template3 = manager.load_template("cached").unwrap();
        assert_eq!(template3, "Modified content");
    }
    
    #[test]
    fn test_disable_cache() {
        // Create a temporary directory for test templates
        let temp_dir = tempdir().unwrap();
        let template_path = temp_dir.path().join("nocache.txt");
        
        // Create a test template file
        let mut file = File::create(&template_path).unwrap();
        file.write_all(b"Original content").unwrap();
        
        // Create template manager with caching disabled
        let manager = TemplateManager::new(temp_dir.path()).disable_cache();
        
        // Load template
        let template1 = manager.load_template("nocache").unwrap();
        assert_eq!(template1, "Original content");
        
        // Modify the file
        let mut file = File::create(&template_path).unwrap();
        file.write_all(b"Modified content").unwrap();
        
        // Load template again (should get updated content despite global cache)
        let template2 = manager.load_template("nocache").unwrap();
        assert_eq!(template2, "Modified content");
    }
}