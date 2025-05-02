//! Tests for the prompt template system.

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::fs;
    use std::path::Path;
    use crate::bidirectional_agent::llm_core::template::TemplateManager;
    
    // Helper function to ensure our test prompt directory exists with test templates
    fn setup_test_prompts() {
        let prompt_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("bidirectional_agent")
            .join("llm_core")
            .join("prompts");
            
        // Create directory if it doesn't exist
        if !prompt_dir.exists() {
            fs::create_dir_all(&prompt_dir).expect("Failed to create prompt directory");
        }
        
        // Test basic rendering
        let test_template = prompt_dir.join("test_basic.txt");
        if !test_template.exists() {
            fs::write(
                test_template,
                "This is a test template for {{name}}.",
            ).expect("Failed to write test template");
        }
        
        // Test complex rendering with multiple variables
        let complex_template = prompt_dir.join("test_complex.txt");
        if !complex_template.exists() {
            fs::write(
                complex_template,
                "# {{title}}\n\nDescription: {{description}}\n\nAuthor: {{author}}\nDate: {{date}}",
            ).expect("Failed to write complex template");
        }
    }
    
    #[test]
    fn test_default_template_dir() {
        // Setup test prompts
        setup_test_prompts();
        
        // Create template manager with default directory
        let manager = TemplateManager::with_default_dir();
        
        // List templates
        let templates = manager.list_templates().expect("Failed to list templates");
        
        // Should include our test templates
        assert!(templates.contains(&"test_basic".to_string()));
        assert!(templates.contains(&"test_complex".to_string()));
    }
    
    #[test]
    fn test_basic_template_rendering() {
        // Setup test prompts
        setup_test_prompts();
        
        // Create template manager with default directory
        let manager = TemplateManager::with_default_dir();
        
        // Create variables
        let mut variables = HashMap::new();
        variables.insert("name".to_string(), "Claude".to_string());
        
        // Render template
        let rendered = manager.render("test_basic", &variables).expect("Failed to render template");
        
        // Check result
        assert_eq!(rendered, "This is a test template for Claude.");
    }
    
    #[test]
    fn test_complex_template_rendering() {
        // Setup test prompts
        setup_test_prompts();
        
        // Create template manager with default directory
        let manager = TemplateManager::with_default_dir();
        
        // Create variables
        let mut variables = HashMap::new();
        variables.insert("title".to_string(), "Test Document".to_string());
        variables.insert("description".to_string(), "This is a test document for template rendering".to_string());
        variables.insert("author".to_string(), "Claude".to_string());
        variables.insert("date".to_string(), "2025-05-01".to_string());
        
        // Render template
        let rendered = manager.render("test_complex", &variables).expect("Failed to render template");
        
        // Check result
        let expected = "# Test Document\n\nDescription: This is a test document for template rendering\n\nAuthor: Claude\nDate: 2025-05-01";
        assert_eq!(rendered, expected);
    }
    
    #[test]
    fn test_missing_variable() {
        // Setup test prompts
        setup_test_prompts();
        
        // Create template manager with default directory
        let manager = TemplateManager::with_default_dir();
        
        // Create variables with missing fields
        let mut variables = HashMap::new();
        variables.insert("title".to_string(), "Test Document".to_string());
        variables.insert("author".to_string(), "Claude".to_string());
        // Missing description and date
        
        // Render should fail
        let result = manager.render("test_complex", &variables);
        assert!(result.is_err());
        
        // Error should mention missing variables
        let err = result.unwrap_err().to_string();
        assert!(err.contains("{{description}}") || err.contains("{{date}}"));
    }
    
    #[test]
    fn test_routing_decision_template() {
        // Setup test prompts
        setup_test_prompts();
        
        // Create template manager
        let manager = TemplateManager::with_default_dir();
        
        // Create variables
        let mut variables = HashMap::new();
        variables.insert("task_description".to_string(), "List all files in the current directory".to_string());
        variables.insert("available_tools".to_string(), "- directory: Query and manage agent directory\n- echo: Simple echo tool\n- ls: List files".to_string());
        variables.insert("available_agents".to_string(), "- file_agent: Specializes in file operations\n- search_agent: Specializes in search operations".to_string());
        
        // Render template
        let rendered = manager.render("routing_decision", &variables).expect("Failed to render routing template");
        
        // Check that template has all the placeholders filled
        assert!(rendered.contains("List all files in the current directory"));
        assert!(rendered.contains("- directory: Query and manage agent directory"));
        assert!(rendered.contains("- file_agent: Specializes in file operations"));
        
        // Check that template markers exist
        assert!(rendered.contains("# Task Routing Decision"));
        assert!(rendered.contains("## Response Format Requirements"));
    }
}