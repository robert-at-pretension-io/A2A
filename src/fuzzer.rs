use serde_json::{self, Value};

// Simplified version of the fuzzing logic for non-fuzzing mode
#[cfg(not(feature = "fuzzing"))]
pub fn run_fuzzer(target: &str, time: u64) {
    println!("🔍 Fuzzing is not enabled. Recompile with --features=fuzzing");
    println!("Target: {}, Time: {} seconds", target, time);
    
    // Perform some basic pseudo-fuzzing with predefined cases
    println!("\nRunning basic test cases instead:");
    
    match target {
        "parse" => {
            println!("Testing JSON parsing with basic cases...");
            let test_cases = [
                r#"{"jsonrpc":"2.0","method":"tasks/send","params":{"id":"123"}}"#,
                r#"{"jsonrpc":"2.0","method":"tasks/get","params":{"id":"456"}}"#,
                r#"{"jsonrpc":"2.0"}"#, // Missing method
                r#"{"method":"tasks/send"}"#, // Missing jsonrpc version
                r#"{}"#, // Empty object
                r#"[]"#, // Array instead of object
                r#""string""#, // String instead of object
                r#"123"#, // Number instead of object
                r#"{"jsonrpc":"2.0","method":123}"#, // Method is not a string
            ];
            
            for (i, test_case) in test_cases.iter().enumerate() {
                println!("\nTest case {}: {}", i + 1, test_case);
                let result = serde_json::from_str::<Value>(test_case);
                match result {
                    Ok(value) => println!("  ✓ Parsed as: {}", value),
                    Err(e) => println!("  ✗ Parse error: {}", e),
                }
            }
        },
        "handle" => {
            println!("Testing request handling with basic cases...");
            let test_cases = [
                r#"{"jsonrpc":"2.0","id":"1","method":"tasks/send","params":{"id":"test-agent-1","message":{"role":"user","parts":[{"type":"text","text":"Hello"}]}}}"#,
                r#"{"jsonrpc":"2.0","id":"2","method":"tasks/get","params":{"id":"task-123"}}"#,
                r#"{"jsonrpc":"2.0","id":"3","method":"unknown_method"}"#,
                r#"{"jsonrpc":"2.0","id":"4","method":"tasks/send","params":{}}"#, // Missing required params
            ];
            
            for (i, test_case) in test_cases.iter().enumerate() {
                println!("\nTest case {}: {}", i + 1, test_case);
                let parsed = serde_json::from_str::<Value>(test_case).unwrap();
                let method = parsed.get("method").and_then(|m| m.as_str()).unwrap_or("none");
                println!("  Method: {}", method);
                
                // Simulate handling
                match method {
                    "tasks/send" => {
                        if parsed.get("params").and_then(|p| p.get("id")).is_some() {
                            println!("  ✓ Valid task send request");
                        } else {
                            println!("  ✗ Invalid parameters - missing id");
                        }
                    },
                    "tasks/get" => {
                        if parsed.get("params").and_then(|p| p.get("id")).is_some() {
                            println!("  ✓ Valid task get request");
                        } else {
                            println!("  ✗ Invalid parameters - missing id");
                        }
                    },
                    _ => println!("  ✗ Unknown method"),
                }
            }
        },
        _ => {
            println!("Unknown fuzzing target: {}", target);
            println!("Available targets: parse, handle");
        }
    }
}

// Fuzzing code that is only included when building with the fuzzing feature
#[cfg(feature = "fuzzing")]
pub fn run_fuzzer(target: &str, _time: u64) {
    use libfuzzer_sys::fuzz_target;
    
    match target {
        "parse" => {
            println!("Fuzzing JSON parsing...");
            fuzz_target!(|data: &[u8]| {
                if let Ok(s) = std::str::from_utf8(data) {
                    let _ = serde_json::from_str::<Value>(s);
                }
            });
        },
        "handle" => {
            println!("Fuzzing request handling...");
            fuzz_target!(|data: &[u8]| {
                if let Ok(s) = std::str::from_utf8(data) {
                    if let Ok(req) = serde_json::from_str::<Value>(s) {
                        // Test handling logic
                        if let Some(method) = req.get("method").and_then(|m| m.as_str()) {
                            match method {
                                "tasks/send" => {
                                    // Check if params.id exists
                                    let _ = req.get("params").and_then(|p| p.get("id"));
                                },
                                "tasks/get" => {
                                    // Check if params.id exists
                                    let _ = req.get("params").and_then(|p| p.get("id"));
                                },
                                _ => {}
                            }
                        }
                    }
                }
            });
        },
        _ => {
            println!("Unknown fuzzing target: {}", target);
            println!("Available targets: parse, handle");
        }
    }
}
