// use arbitrary::{Arbitrary, Unstructured}; // Unused
use rand::{Rng, thread_rng};
use std::time::{Duration, Instant};
use crate::types::*;
use crate::validator;
use serde_json::{self, json, Value};
use std::panic;

// Simple arbitrary implementation for A2A objects
#[derive(Debug, Clone)]
struct FuzzTask {
    id: Option<String>,
    session_id: Option<String>,
    status: FuzzTaskStatus,
    history: Option<Vec<FuzzMessage>>,
    artifacts: Option<Vec<FuzzArtifact>>,
}

#[derive(Debug, Clone)]
struct FuzzTaskStatus {
    state: String,
    message: Option<FuzzMessage>,
    timestamp: Option<String>,
}

#[derive(Debug, Clone)]
struct FuzzMessage {
    role: String,
    parts: Vec<FuzzPart>,
}

#[derive(Debug, Clone)]
enum FuzzPart {
    Text(String),
    File(String, Option<String>), // filename, content
    Data(Value),
    Invalid(String), // invalid type
}

#[derive(Debug, Clone)]
struct FuzzArtifact {
    name: Option<String>,
    parts: Vec<FuzzPart>,
    index: Option<i32>,
}

#[derive(Debug, Clone)]
struct FuzzJsonRpc {
    jsonrpc: Option<String>,
    id: Option<Value>,
    method: Option<String>,
    params: Option<Value>,
}

// Generate random strings, with occasional invalid ones
fn random_string(rng: &mut impl Rng) -> String {
    if rng.gen_ratio(1, 20) {
        // Occasionally return invalid strings
        match rng.gen_range(0..3) {
            0 => "".to_string(),
            1 => "null".to_string(),
            _ => "undefined".to_string(),
        }
    } else {
        // Most of the time return valid-looking strings
        let len = rng.gen_range(5..15);
        let mut s = String::with_capacity(len);
        let charset = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-_";
        for _ in 0..len {
            s.push(charset.chars().nth(rng.gen_range(0..charset.len())).unwrap());
        }
        s
    }
}

// Generate random task state
fn random_task_state(rng: &mut impl Rng) -> String {
    if rng.gen_ratio(1, 10) {
        // Occasionally return invalid state
        random_string(rng)
    } else {
        // Most of the time return valid state
        let states = ["submitted", "working", "input-required", 
                      "completed", "canceled", "failed", "unknown"];
        states[rng.gen_range(0..states.len())].to_string()
    }
}

// Generate random role
fn random_role(rng: &mut impl Rng) -> String {
    if rng.gen_ratio(1, 10) {
        // Occasionally return invalid role
        match rng.gen_range(0..3) {
            0 => "system".to_string(),
            1 => "assistant".to_string(),
            _ => random_string(rng),
        }
    } else {
        // Most of the time return valid role
        if rng.gen_bool(0.5) { "user".to_string() } else { "agent".to_string() }
    }
}

// Generate random A2A components with controlled chaos
impl FuzzTask {
    fn random(rng: &mut impl Rng) -> Self {
        Self {
            id: if rng.gen_bool(0.9) { Some(random_string(rng)) } else { None },
            session_id: if rng.gen_bool(0.8) { Some(random_string(rng)) } else { None },
            status: FuzzTaskStatus::random(rng),
            history: if rng.gen_bool(0.7) {
                Some((0..rng.gen_range(0..3)).map(|_| FuzzMessage::random(rng)).collect())
            } else { None },
            artifacts: if rng.gen_bool(0.7) {
                Some((0..rng.gen_range(0..2)).map(|_| FuzzArtifact::random(rng)).collect())
            } else { None },
        }
    }

    fn to_json(&self) -> Value {
        let mut obj = serde_json::Map::new();
        
        if let Some(id) = &self.id {
            obj.insert("id".to_string(), json!(id));
        }
        
        if let Some(session_id) = &self.session_id {
            obj.insert("sessionId".to_string(), json!(session_id));
        }
        
        // Add status
        obj.insert("status".to_string(), self.status.to_json());
        
        // Add history if present
        if let Some(history) = &self.history {
            let history_array = history.iter().map(|msg| msg.to_json()).collect::<Vec<_>>();
            obj.insert("history".to_string(), json!(history_array));
        }
        
        // Add artifacts if present
        if let Some(artifacts) = &self.artifacts {
            let artifacts_array = artifacts.iter().map(|art| art.to_json()).collect::<Vec<_>>();
            obj.insert("artifacts".to_string(), json!(artifacts_array));
        }
        
        json!(obj)
    }
}

impl FuzzTaskStatus {
    fn random(rng: &mut impl Rng) -> Self {
        Self {
            state: random_task_state(rng),
            message: if rng.gen_bool(0.5) { Some(FuzzMessage::random(rng)) } else { None },
            timestamp: if rng.gen_bool(0.8) {
                Some(format!("2023-{:02}-{:02}T{:02}:{:02}:{:02}Z", 
                    rng.gen_range(1..=12), 
                    rng.gen_range(1..=28), 
                    rng.gen_range(0..=23),
                    rng.gen_range(0..=59),
                    rng.gen_range(0..=59)))
            } else { None },
        }
    }

    fn to_json(&self) -> Value {
        let mut obj = serde_json::Map::new();
        
        obj.insert("state".to_string(), json!(self.state));
        
        if let Some(message) = &self.message {
            obj.insert("message".to_string(), message.to_json());
        }
        
        if let Some(timestamp) = &self.timestamp {
            obj.insert("timestamp".to_string(), json!(timestamp));
        }
        
        json!(obj)
    }
}

impl FuzzMessage {
    fn random(rng: &mut impl Rng) -> Self {
        let num_parts = rng.gen_range(1..=3);
        Self {
            role: random_role(rng),
            parts: (0..num_parts).map(|_| FuzzPart::random(rng)).collect(),
        }
    }

    fn to_json(&self) -> Value {
        let parts_array = self.parts.iter().map(|part| part.to_json()).collect::<Vec<_>>();
        
        json!({
            "role": self.role,
            "parts": parts_array
        })
    }
}

impl FuzzPart {
    fn random(rng: &mut impl Rng) -> Self {
        match rng.gen_range(0..=3) {
            0 => FuzzPart::Text(random_string(rng)),
            1 => FuzzPart::File(
                random_string(rng),
                if rng.gen_bool(0.5) { Some(format!("base64:{}", random_string(rng))) } else { None }
            ),
            2 => FuzzPart::Data(json!({
                "key1": random_string(rng),
                "key2": rng.gen_range(1..100),
                "key3": rng.gen_bool(0.5),
            })),
            _ => FuzzPart::Invalid(random_string(rng)),
        }
    }

    fn to_json(&self) -> Value {
        match self {
            FuzzPart::Text(text) => {
                json!({
                    "type": "text",
                    "text": text
                })
            },
            FuzzPart::File(name, content) => {
                let mut file_obj = serde_json::Map::new();
                file_obj.insert("name".to_string(), json!(name));
                
                if let Some(bytes) = content {
                    file_obj.insert("bytes".to_string(), json!(bytes));
                } else {
                    file_obj.insert("uri".to_string(), json!(format!("https://example.com/{}", name)));
                }
                
                json!({
                    "type": "file",
                    "file": file_obj
                })
            },
            FuzzPart::Data(data) => {
                json!({
                    "type": "data",
                    "data": data
                })
            },
            FuzzPart::Invalid(invalid_type) => {
                json!({
                    "type": invalid_type,
                    "unknown_field": "some value"
                })
            },
        }
    }
}

impl FuzzArtifact {
    fn random(rng: &mut impl Rng) -> Self {
        let num_parts = rng.gen_range(1..=2);
        Self {
            name: if rng.gen_bool(0.8) { Some(random_string(rng)) } else { None },
            parts: (0..num_parts).map(|_| FuzzPart::random(rng)).collect(),
            index: if rng.gen_bool(0.8) { Some(rng.gen_range(0..5)) } else { None },
        }
    }

    fn to_json(&self) -> Value {
        let mut obj = serde_json::Map::new();
        
        if let Some(name) = &self.name {
            obj.insert("name".to_string(), json!(name));
        }
        
        let parts_array = self.parts.iter().map(|part| part.to_json()).collect::<Vec<_>>();
        obj.insert("parts".to_string(), json!(parts_array));
        
        if let Some(index) = self.index {
            obj.insert("index".to_string(), json!(index));
        }
        
        json!(obj)
    }
}

impl FuzzJsonRpc {
    fn random(rng: &mut impl Rng) -> Self {
        let methods = ["tasks/send", "tasks/get", "tasks/cancel", 
                      "tasks/pushNotification/set", "tasks/pushNotification/get",
                      "tasks/sendSubscribe", "tasks/resubscribe", "invalid/method"];
        
        Self {
            jsonrpc: if rng.gen_bool(0.9) { Some("2.0".to_string()) } 
                    else { Some(format!("{}.0", rng.gen_range(1..=5))) },
            id: if rng.gen_bool(0.9) { 
                     if rng.gen_bool(0.5) { 
                         Some(json!(random_string(rng))) 
                     } else { 
                         Some(json!(rng.gen_range(1..1000)))
                     }
                 } else { 
                     None 
                 },
            method: if rng.gen_bool(0.9) { 
                     Some(methods[rng.gen_range(0..methods.len())].to_string()) 
                 } else { 
                     None 
                 },
            params: if rng.gen_bool(0.9) {
                     let task = FuzzTask::random(rng);
                     Some(json!({ "id": task.id.unwrap_or_else(|| random_string(rng)) }))
                 } else {
                     None
                 },
        }
    }

    fn to_json(&self) -> Value {
        let mut obj = serde_json::Map::new();
        
        if let Some(version) = &self.jsonrpc {
            obj.insert("jsonrpc".to_string(), json!(version));
        }
        
        if let Some(id) = &self.id {
            obj.insert("id".to_string(), id.clone());
        }
        
        if let Some(method) = &self.method {
            obj.insert("method".to_string(), json!(method));
        }
        
        if let Some(params) = &self.params {
            obj.insert("params".to_string(), params.clone());
        }
        
        json!(obj)
    }
}

/// Run the fuzzer with specified target, time limit, and optional output
pub fn run_fuzzer(target: &str, time_seconds: u64) {
    let start_time = Instant::now();
    let end_time = start_time + Duration::from_secs(time_seconds);
    let mut rng = thread_rng();
    
    let mut tests_run = 0;
    let mut failures = 0;
    
    println!("🔍 Fuzzing target '{}' for {} seconds", target, time_seconds);
    
    match target {
        "schema" => {
            println!("Fuzzing A2A schema validation...");
            
            while Instant::now() < end_time {
                tests_run += 1;
                let task = FuzzTask::random(&mut rng);
                let json = task.to_json();
                
                // Use panic::catch_unwind to detect crashes
                let result = panic::catch_unwind(|| {
                    let _ = validator::validate_json(&json);
                });
                
                if result.is_err() {
                    failures += 1;
                    println!("💥 Found a crash with input: {}", json);
                }
                
                // Status update every 1000 tests
                if tests_run % 1000 == 0 {
                    let elapsed = start_time.elapsed().as_secs();
                    println!("Ran {} tests ({}/sec), found {} failures", 
                             tests_run, tests_run / (elapsed + 1), failures);
                }
            }
        },
        "jsonrpc" => {
            println!("Fuzzing JSON-RPC request handling...");
            
            while Instant::now() < end_time {
                tests_run += 1;
                let request = FuzzJsonRpc::random(&mut rng);
                let json = request.to_json();
                
                // Use serde_json to test parsing resilience
                let result = panic::catch_unwind(|| {
                    // Tests JSON-RPC parsing (would be better with direct handler access)
                    let _ = json.get("jsonrpc");
                    let _ = json.get("method");
                    let _ = json.get("params");
                });
                
                if result.is_err() {
                    failures += 1;
                    println!("💥 Found a crash with input: {}", json);
                }
                
                // Status update every 1000 tests
                if tests_run % 1000 == 0 {
                    let elapsed = start_time.elapsed().as_secs();
                    println!("Ran {} tests ({}/sec), found {} failures", 
                             tests_run, tests_run / (elapsed + 1), failures);
                }
            }
        },
        "message" => {
            println!("Fuzzing A2A message parsing...");
            
            while Instant::now() < end_time {
                tests_run += 1;
                let message = FuzzMessage::random(&mut rng);
                let json = message.to_json();
                
                // Test deserializing as a real Message
                let result = panic::catch_unwind(|| {
                    let _ = serde_json::from_value::<Message>(json.clone());
                });
                
                if result.is_err() {
                    failures += 1;
                    println!("💥 Found a crash with input: {}", json);
                }
                
                // Status update every 1000 tests
                if tests_run % 1000 == 0 {
                    let elapsed = start_time.elapsed().as_secs();
                    println!("Ran {} tests ({}/sec), found {} failures", 
                             tests_run, tests_run / (elapsed + 1), failures);
                }
            }
        },
        _ => {
            println!("Unknown target: {}. Available targets: schema, jsonrpc, message", target);
            println!("Defaulting to schema fuzzing...");
            run_fuzzer("schema", time_seconds);
        }
    }
    
    let elapsed = start_time.elapsed();
    println!("🏁 Fuzzing complete:");
    println!("   Total tests: {}", tests_run);
    println!("   Tests per second: {}", tests_run / elapsed.as_secs().max(1));
    println!("   Failures found: {}", failures);
    println!("   Time elapsed: {:.2}s", elapsed.as_secs_f64());
}
