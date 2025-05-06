use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use std::error::Error;
use std::fs::File;
use std::io::Write;
use std::path::Path;

use crate::client::A2aClient;
use crate::types::{Artifact, Part, Task, TaskQueryParams};

/// Type representing the result of saving an artifact
pub struct SavedArtifact {
    pub name: String,
    pub path: String,
    pub mime_type: Option<String>,
}

impl A2aClient {
    /// Get a task's artifacts (if any)
    pub async fn get_task_artifacts(
        &mut self,
        task_id: &str,
    ) -> Result<Vec<Artifact>, Box<dyn Error>> {
        // Fetch the task
        let task = self.get_task(task_id).await?;

        // Return the artifacts (if any)
        Ok(task.artifacts.unwrap_or_default())
    }

    /// Get a specific artifact by task ID and artifact index
    pub async fn get_task_artifact(
        &mut self,
        task_id: &str,
        artifact_index: u64,
    ) -> Result<Option<Artifact>, Box<dyn Error>> {
        // Create request parameters using TaskQueryParams
        let params = TaskQueryParams {
            id: task_id.to_string(),
            history_length: None,
            metadata: None,
        };

        // Get the full task
        let task: Task = self
            .send_jsonrpc("tasks/get", serde_json::to_value(params)?)
            .await?;

        // Find the specific artifact by index
        if let Some(artifacts) = task.artifacts {
            return Ok(artifacts
                .into_iter()
                .find(|a| a.index == artifact_index as i64));
        }

        Ok(None)
    }

    /// Save artifact files to the specified directory
    pub fn save_artifacts(
        &self,
        artifacts: &[Artifact],
        output_dir: &str,
    ) -> Result<Vec<SavedArtifact>, Box<dyn Error>> {
        let mut saved_artifacts = Vec::new();

        // Create output directory if it doesn't exist
        std::fs::create_dir_all(output_dir)?;

        for (i, artifact) in artifacts.iter().enumerate() {
            // Use artifact name if available, otherwise generate one
            let artifact_name = artifact
                .name
                .clone()
                .unwrap_or_else(|| format!("artifact_{}", i));

            for (j, part) in artifact.parts.iter().enumerate() {
                match part {
                    Part::FilePart(file_part) => {
                        // Handle file parts
                        let file_name = file_part
                            .file
                            .name
                            .clone()
                            .unwrap_or_else(|| format!("{}_{}", artifact_name, j));

                        let mime_type = file_part.file.mime_type.clone();

                        let output_path = Path::new(output_dir).join(&file_name);
                        let path_str = output_path.to_string_lossy().to_string();

                        // Save the file content
                        if let Some(bytes) = &file_part.file.bytes {
                            // Decode base64 content and write to file
                            let decoded = BASE64.decode(bytes)?;
                            let mut file = File::create(&output_path)?;
                            file.write_all(&decoded)?;

                            saved_artifacts.push(SavedArtifact {
                                name: file_name,
                                path: path_str,
                                mime_type,
                            });
                        } else if let Some(uri) = &file_part.file.uri {
                            // Just record URI reference without downloading
                            saved_artifacts.push(SavedArtifact {
                                name: file_name,
                                path: uri.clone(),
                                mime_type,
                            });
                        }
                    }
                    Part::TextPart(text_part) => {
                        // Save text content to file
                        let file_name = format!("{}_{}.txt", artifact_name, j);
                        let output_path = Path::new(output_dir).join(&file_name);
                        let path_str = output_path.to_string_lossy().to_string();

                        let mut file = File::create(&output_path)?;
                        file.write_all(text_part.text.as_bytes())?;

                        saved_artifacts.push(SavedArtifact {
                            name: file_name,
                            path: path_str,
                            mime_type: Some("text/plain".to_string()),
                        });
                    }
                    Part::DataPart(data_part) => {
                        // Save data as JSON file
                        let file_name = format!("{}_{}.json", artifact_name, j);
                        let output_path = Path::new(output_dir).join(&file_name);
                        let path_str = output_path.to_string_lossy().to_string();

                        let json_str = serde_json::to_string_pretty(&data_part.data)?;
                        let mut file = File::create(&output_path)?;
                        file.write_all(json_str.as_bytes())?;

                        saved_artifacts.push(SavedArtifact {
                            name: file_name,
                            path: path_str,
                            mime_type: Some("application/json".to_string()),
                        });
                    }
                }
            }
        }

        Ok(saved_artifacts)
    }

    /// Extract all text content from artifacts
    pub fn extract_artifact_text(artifacts: &[Artifact]) -> Vec<String> {
        let mut texts = Vec::new();

        for artifact in artifacts {
            for part in &artifact.parts {
                if let Part::TextPart(text_part) = part {
                    texts.push(text_part.text.clone());
                }
            }
        }

        texts
    }

    /// Extract all file content from artifacts
    pub fn extract_artifact_files(artifacts: &[Artifact]) -> Vec<(String, Option<Vec<u8>>)> {
        let mut files = Vec::new();

        for artifact in artifacts {
            for part in &artifact.parts {
                if let Part::FilePart(file_part) = part {
                    // File name and content as tuple
                    let name = file_part
                        .file
                        .name
                        .as_deref()
                        .unwrap_or("unnamed_file")
                        .to_string();
                    let bytes = file_part
                        .file
                        .bytes
                        .as_deref()
                        .and_then(|b| BASE64.decode(b).ok());

                    if let Some(bytes) = bytes {
                        files.push((name, Some(bytes)));
                    } else if file_part.file.uri.is_some() {
                        // For URI references, we don't have bytes
                        files.push((name, None));
                    }
                }
            }
        }

        files
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{FileContent, FilePart, TextPart};
    use mockito::Server;
    use serde_json::json;
    use tempfile::tempdir;
    use tokio::test;

    #[test]
    async fn test_get_task_artifacts() {
        // Arrange
        let task_id = "task-with-artifacts-123";
        let response = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": {
                "id": task_id,
                "sessionId": "session-456",
                "status": {
                    "state": "completed",
                    "timestamp": "2023-01-01T00:00:00Z"
                },
                "artifacts": [
                    {
                        "name": "result",
                        "index": 0,
                        "parts": [
                            {
                                "type": "text",
                                "text": "This is a text artifact"
                            }
                        ]
                    },
                    {
                        "name": "data_output",
                        "index": 1,
                        "parts": [
                            {
                                "type": "data",
                                "data": {
                                    "result": "success",
                                    "count": 42
                                }
                            }
                        ]
                    }
                ]
            }
        });

        let mut server = Server::new_async().await;

        // Mock the tasks/get endpoint
        let mock = server
            .mock("POST", "/")
            .with_status(200)
            .with_header("content-type", "application/json")
            .match_body(mockito::Matcher::PartialJson(json!({
                "jsonrpc": "2.0",
                "method": "tasks/get",
                "params": {
                    "id": task_id
                }
            })))
            .with_body(response.to_string())
            .create_async()
            .await;

        // Act
        let mut client = A2aClient::new(&server.url());
        let artifacts = client.get_task_artifacts(task_id).await.unwrap();

        // Assert
        assert_eq!(artifacts.len(), 2);
        assert_eq!(artifacts[0].name.as_ref().unwrap(), "result");
        assert_eq!(artifacts[1].index, 1);

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_save_artifacts() {
        // Create test artifacts
        let text_part = TextPart {
            type_: "text".to_string(),
            text: "This is artifact text content".to_string(),
            metadata: None,
        };

        // Create a temporary directory for testing
        let temp_dir = tempdir().unwrap();
        let output_dir = temp_dir.path().to_str().unwrap();

        // Create test artifacts
        let artifacts = vec![Artifact {
            name: Some("test_artifact".to_string()),
            index: 0,
            parts: vec![Part::TextPart(text_part)],
            append: None,
            description: Some("Test artifact".to_string()),
            last_chunk: None,
            metadata: None,
        }];

        // Save artifacts
        let client = A2aClient::new("http://example.com");
        let saved = client.save_artifacts(&artifacts, output_dir).unwrap();

        // Verify saved artifacts
        assert_eq!(saved.len(), 1);
        assert!(saved[0].name.contains("test_artifact"));
        assert!(saved[0].path.contains(output_dir));

        // Check the file exists
        let artifact_path = &saved[0].path;
        assert!(std::fs::metadata(artifact_path).is_ok());

        // Clean up the temporary directory
        temp_dir.close().unwrap();
    }

    #[tokio::test]
    async fn test_extract_artifact_text() {
        // Create test artifacts with text
        let text_part1 = TextPart {
            type_: "text".to_string(),
            text: "First text content".to_string(),
            metadata: None,
        };

        let text_part2 = TextPart {
            type_: "text".to_string(),
            text: "Second text content".to_string(),
            metadata: None,
        };

        let artifacts = vec![
            Artifact {
                name: Some("artifact1".to_string()),
                index: 0,
                parts: vec![Part::TextPart(text_part1)],
                append: None,
                description: None,
                last_chunk: None,
                metadata: None,
            },
            Artifact {
                name: Some("artifact2".to_string()),
                index: 1,
                parts: vec![Part::TextPart(text_part2)],
                append: None,
                description: None,
                last_chunk: None,
                metadata: None,
            },
        ];

        // Extract text
        let texts = A2aClient::extract_artifact_text(&artifacts);

        // Verify extracted text
        assert_eq!(texts.len(), 2);
        assert_eq!(texts[0], "First text content");
        assert_eq!(texts[1], "Second text content");
    }
}
