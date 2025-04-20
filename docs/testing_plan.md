# A2A Server Testing Framework Plan

## 1. Compliance Testing Objectives

### Core Protocol Compliance
- Schema Validation: Ensure all responses strictly adhere to the A2A JSON schema
- HTTP/JSON-RPC Compliance: Validate proper implementation of JSON-RPC 2.0 specifications
- Status Code Usage: Verify appropriate HTTP status codes for different scenarios

### Feature Coverage Assessment
- **Capability Detection**: Test server's ability to accurately report its capabilities
- **Core Operation Support**: Validate implementation of all required API endpoints
- **Optional Feature Testing**: Test streaming, push notifications, and structured output if supported

### Error Handling & Resilience
- **Error Responses**: Verify correct implementation of JSON-RPC error codes and messages
- **Error Recovery**: Test server recovery after receiving invalid requests
- **Rate Limiting Behavior**: Observe and document rate limiting implementations

## 2. Test Categories

### Schema Compliance Tests
- Validate AgentCard structure and accessibility
- Verify all endpoint responses match schema definitions
- Test JSON serialization/deserialization of all object types

### Endpoint-Specific Tests
For each endpoint (`tasks/send`, `tasks/get`, etc.):
- Valid request processing
- Invalid parameter handling
- Authentication requirements
- Response format validation
- Performance under load

### Multi-turn Conversation Tests
- Track conversation state through multiple interactions
- Test agent memory and context maintenance
- Verify proper state transitions
- Test conversation resumption capabilities

### Streaming Tests (if supported)
- Verify SSE implementation correctness
- Test partial response handling
- Measure streaming latency and throughput
- Test client disconnect and reconnect scenarios

### Push Notification Tests (if supported)
- Validate callback implementation
- Test notification security mechanisms
- Verify notification delivery reliability
- Test configuration updates

### Authentication Tests
- Test all supported authentication schemes
- Verify token refresh mechanisms
- Test unauthorized access attempts
- Validate credential security

## 3. Test Methodologies

### Functional Testing
- **Basic Compliance**: Verify baseline functionality against the protocol spec
- **Boundary Testing**: Test limits of message sizes, number of parts, etc.
- **Negative Testing**: Send invalid requests to test error handling

### Property-Based Testing
- Generate random valid A2A messages to test handler robustness
- Create randomized task sequences and verify correct state transitions
- Use existing proptest framework to create arbitrary message patterns

### Fuzzing
- Content fuzzing: Malformed JSON, oversized payloads, invalid UTF-8
- Protocol fuzzing: Invalid method names, missing required fields
- Security fuzzing: Injection attempts, overflow tests

### Performance Testing
- Measure response times under various loads
- Test concurrent task processing capabilities
- Evaluate memory usage during long conversations
- Benchmark streaming performance

### Interoperability Testing
- Test clients from different language ecosystems
- Verify seamless operation with various client libraries
- Cross-implementation testing with different A2A server implementations

## 4. Testing Infrastructure Components

### Request Generator
- Create valid and invalid requests for all endpoints
- Support for generating all task states and role types
- Template system for quick test case creation
- Library of corner cases and edge cases

### Response Validator
- Schema validation against A2A JSON schema
- Semantic validation of response content
- Error response classifier
- Response timing analyzer

### Task State Manager
- Track expected task states throughout test scenarios
- Verify state transitions match protocol expectations
- Detect inconsistent or invalid state changes

### Conversation Simulator
- Create realistic multi-turn conversation scenarios
- Simulate user behavior patterns
- Generate complex message structures
- Track conversation history and context

### A2A Client Implementation
- ✅ Implemented robust A2A client with modular design
- ✅ Support for core operations (agent discovery, task management)
- ✅ Error handling and response parsing
- ✅ Authentication capabilities
- ✅ Support for streaming responses
- ✅ Support for push notifications
- ✅ Structured data handling
- ✅ File operations (upload, download, attach)
- ✅ Batch task operations
- ✅ Agent skills discovery and invocation
- ✅ State transition history and metrics

### Mock Server Implementation
- ✅ Implemented basic mock server for testing
- ✅ Support for agent card discovery
- ✅ Support for core task operations (send, get, cancel)
- ✅ Streaming support
- ✅ Push notification support
- ✅ Structured data handling
- ✅ File operations
- ✅ Batch operations
- ✅ Skills API

## 5. Implementation Plan

### Phase 1: Core Compliance Testing
- Implement schema validation using existing validator
- Develop basic request generator for all endpoints
- Create test suites for each endpoint's basic functionality
- Implement authentication testing framework

### Phase 2: Advanced Feature Testing
- Develop streaming test components
- Implement push notification test framework
- Create multi-turn conversation test scenarios
- Add property-based test generators

### Phase 3: Performance and Security Testing
- Implement load testing framework
- Enhance fuzzer for A2A-specific security testing
- Develop benchmark suite for performance metrics
- Create interoperability test suite

### Phase 4: Reporting and Analysis
- Design compliance report format
- Implement test result analyzer
- Create visualization for test coverage
- Develop conformance certification process

## 6. Specific Test Cases

### Core Operation Tests
1. ✅ Agent discovery - test /.well-known/agent.json retrieval
2. ✅ Task creation with various input types (text, file, data)
3. ✅ Task status retrieval (all states)
4. ✅ Task cancellation (at different stages)
5. ✅ Push notification configuration management
6. ✅ Task resubscription and state retrieval
7. ✅ Batch task creation and management
8. ✅ Agent skills discovery and invocation
9. ✅ File operations (upload, download, attachment)
10. ✅ State history tracking and metrics

### Error Handling Tests
1. ✅ Invalid JSON-RPC requests (via fuzzing)
2. ✅ Missing required parameters (via fuzzing)
3. ✅ Invalid task IDs (via fuzzing)
4. ✅ Unauthorized access attempts (via auth testing)
5. ✅ Malformed message content (via fuzzing)
6. ✅ Basic schema validation testing

### Advanced Feature Tests
1. ✅ Multi-part messages with mixed content types
2. ✅ Long-running tasks with state transitions
3. ✅ Structured output requests with schema validation
4. ✅ Streaming with partial response aggregation
5. ✅ Authentication token refresh and expiration
6. ✅ Concurrent task processing via batch operations
7. ✅ Skill invocation with different input/output modes

## 7. Success Criteria

### Compliance Levels
- ✅ **Basic Compliance**: Core endpoints work correctly with proper error handling
- ✅ **Standard Compliance**: All required features plus basic streaming support
- ✅ **Full Compliance**: All features including streaming, push notifications, structured output, file operations, batch processing, and skills API

### Security Requirements
- ✅ Proper authentication implementation
- ✅ Resilience against malformed inputs (via fuzzing)
- ✅ Schema validation for all messages

## 8. Implementation Strategy and Progress

### Completed Components
1. ✅ Schema validation module for verifying protocol compliance
2. ✅ Property testing framework for generating A2A-specific test cases
3. ✅ Mock server implementation for testing client behavior
4. ✅ A2A client implementation with complete feature support:
   - ✅ Core operations (discovery, task management)
   - ✅ Streaming capability
   - ✅ Push notifications
   - ✅ Structured data handling
   - ✅ File operations
   - ✅ Task batch operations
   - ✅ Agent skills discovery and invocation
   - ✅ State transition history tracking
5. ✅ Integration testing infrastructure with automated test scenarios
6. ✅ Command-line interface for all client features
7. ✅ Basic fuzzing capabilities

### Completed Implementations
8. ✅ Enhanced artifact management CLI commands (get-artifacts) 
9. ✅ Enhanced fuzzing capabilities for schema validation and JSON-RPC handling

### Optional Future Enhancements
- Integration with standard Rust fuzzing ecosystem (cargo-fuzz)
- Performance benchmarking for A2A operations

This testing framework provides a comprehensive approach to validating A2A server implementations, ensuring they fully comply with the protocol specifications and offer robust, high-performance agent capabilities. With the addition of the client implementation, the test suite now offers a complete solution for both testing server implementations and providing a reference client implementation.

## Error Handling Improvements for Mock Server

The following changes should be made to align the mock server's error handling with the A2A schema:

1. **Define Error Code Constants**:
   ```rust
   // JSON-RPC standard error codes
   const ERROR_PARSE: i64 = -32700;             // "Invalid JSON payload"
   const ERROR_INVALID_REQUEST: i64 = -32600;   // "Request payload validation error"
   const ERROR_METHOD_NOT_FOUND: i64 = -32601;  // "Method not found"
   const ERROR_INVALID_PARAMS: i64 = -32602;    // "Invalid parameters"
   const ERROR_INTERNAL: i64 = -32603;          // "Internal error"
   
   // A2A-specific error codes
   const ERROR_TASK_NOT_FOUND: i64 = -32001;    // "Task not found"
   const ERROR_TASK_NOT_CANCELABLE: i64 = -32002; // "Task cannot be canceled"
   const ERROR_PUSH_NOT_SUPPORTED: i64 = -32003; // "Push Notification is not supported"
   const ERROR_UNSUPPORTED_OP: i64 = -32004;    // "This operation is not supported"
   const ERROR_INCOMPATIBLE_TYPES: i64 = -32005; // "Incompatible content types"
   ```

2. **Fix Authentication Error Handling**: 
   - Use HTTP 401 status with a standard JSON-RPC error structure
   - Currently using -32001 incorrectly which should be for "Task not found"

3. **Standardize Error Messages**:
   - Use exact error messages from the schema
   - For file operations, use appropriate custom error codes (-32000 range)

4. **Implement Missing Error Types**:
   - Add handlers for all A2A-specific error codes
   - Create proper error response functions to ensure consistency

5. **Create Error Helper Function**:
   ```rust
   fn create_error_response(id: Option<&Value>, code: i64, message: &str, data: Option<Value>) -> Value {
       let mut error = json!({
           "code": code,
           "message": message
       });
       
       if let Some(error_data) = data {
           if let Some(obj) = error.as_object_mut() {
               obj.insert("data".to_string(), error_data);
           }
       }
       
       json!({
           "jsonrpc": "2.0",
           "id": id.unwrap_or(&Value::Null),
           "error": error
       })
   }
   ```