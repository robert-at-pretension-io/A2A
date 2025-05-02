# Phase 1 Design: Remote Tool Execution Protocol

## Overview

This document outlines the design for completing the remote tool execution path in the A2A test suite. After reviewing the existing codebase, we can see that there's a foundation for remote tool execution in `src/bidirectional_agent/tools/pluggable/mod.rs`, but it needs refinement and completion to enable proper remote tool discovery, execution, and result processing within the A2A protocol.

## Current Implementation State

The current implementation includes:

1. **RemoteToolRegistry**: A basic structure to track tools exposed by remote agents. It has:
   - A method to update tools from an agent card
   - A method to update tools from an agent with specific tool names
   - Test-only method to register tools

2. **RemoteToolExecutor**: An executor for remote tools that:
   - Takes a ClientManager to communicate with remote agents
   - Has an execute_tool method that sends tasks to remote agents
   - Uses a simple text-based protocol embedded in task metadata

3. **Feature flags**: Remote tools are gated behind the `bidir-delegate` feature flag

The current implementation is functional but limited. It lacks:
- A formalized protocol for tool discovery
- Proper integration with the A2A client
- Consistent error handling
- Testing and documentation

## Design Approach

Rather than creating a completely new protocol, we'll enhance the existing implementation while maintaining compatibility with the A2A protocol. This approach ensures we're leveraging the existing A2A client capabilities while adding the remote tool functionality.

### 1. Enhanced RemoteToolRegistry

The enhanced RemoteToolRegistry will:

1. **Track remote tools** with complete metadata:
   ```rust
   pub struct RemoteToolInfo {
       pub name: String,
       pub description: String,
       pub agent_id: String,
       pub agent_url: String,
       pub capabilities: Vec<String>,
       pub parameters_schema: Option<Value>,
       pub return_schema: Option<Value>,
       pub last_updated: chrono::DateTime<chrono::Utc>,
   }
   ```

2. **Discover tools** through multiple methods:
   - Parsing AgentCard's skills section for compatible tools
   - Explicit registration API for testing and manual configuration
   - Periodic discovery via directory queries (optional)

3. **Manage tool metadata** with:
   - Versioning support
   - Capability filtering
   - Cache invalidation

### 2. Enhanced RemoteToolExecutor

The RemoteToolExecutor will be enhanced to:

1. **Standardize tool call protocol**:
   ```rust
   pub struct RemoteToolCall {
       pub tool_id: String,
       pub agent_id: String,
       pub params: Value,
       pub timeout_ms: Option<u64>,
       pub metadata: Option<Value>,
   }
   
   pub struct RemoteToolResult {
       pub result: Value,
       pub execution_time_ms: u64,
       pub metadata: Option<Value>,
   }
   ```

2. **Improve error handling**:
   - Distinguish between connection errors and tool execution errors
   - Provide structured error responses
   - Support retries with configurable policies

3. **Support for streaming results**:
   - Extend to support streaming tool execution for long-running tools
   - Add cancellation support for tool executions in progress

4. **Result conversion**:
   - Add utility functions to convert between A2A artifacts and tool results
   - Provide type-safe access to tool results

### 3. A2A Protocol Integration

We'll formalize how tools are represented in the A2A protocol:

1. **Tool Discovery Protocol**:
   - Agent cards will expose tools as specialized skills with:
     ```json
     {
       "id": "tool-NAME",
       "name": "Tool Name",
       "description": "Tool description",
       "tags": ["tool", "category1", "category2"],
       "input_schema": { ... JSON Schema ... },
       "output_schema": { ... JSON Schema ... }
     }
     ```

2. **Tool Invocation Protocol**:
   - Use the existing task structure with tool-specific metadata:
     ```json
     {
       "metadata": {
         "_tool_call": {
           "name": "tool_name",
           "params": { ... parameters ... }
         }
       }
     }
     ```

3. **Tool Response Protocol**:
   - Tool responses will be encoded in the artifact structure:
     ```json
     {
       "metadata": {
         "_tool_result": true,
         "tool_name": "tool_name",
         "execution_time_ms": 123
       },
       "parts": [
         {
           "type": "data",
           "data": { ... result data ... }
         }
       ]
     }
     ```

### 4. Client-Side Integration

We'll enhance the existing A2A client to support tool operations:

1. **New Client Methods**:
   ```rust
   // Discover available tools
   pub async fn list_remote_tools(&self, agent_id: &str) -> Result<Vec<RemoteToolInfo>>;
   
   // Execute a tool
   pub async fn execute_remote_tool(
       &self,
       agent_id: &str,
       tool_name: &str,
       params: Value
   ) -> Result<Value>;
   
   // Execute a tool with streaming results
   pub async fn execute_remote_tool_streaming(
       &self,
       agent_id: &str,
       tool_name: &str,
       params: Value
   ) -> Result<impl Stream<Item = Result<ToolResultChunk, ClientError>>>;
   ```

2. **Tool Discovery Integration**:
   - Extend the existing skill discovery in the A2A client
   - Add tool-specific metadata parsing

3. **Error Handling Integration**:
   - Map tool errors to client errors with appropriate context
   - Add retry logic with configurability

### 5. ClientManager Integration

We'll enhance the ClientManager to properly support the tool execution protocol:

1. **Improved send_task method**:
   - Complete the stub implementation in ClientManager::send_task
   - Add proper error handling and retries
   - Support for tool-specific metadata

2. **Tool Result Processing**:
   - Add utility methods to extract tool results from task responses
   - Convert between tool-specific formats and generic formats

## Implementation Plan

### Phase 1a: Core Infrastructure (Week 1)

1. **Enhanced RemoteToolRegistry**:
   - Complete the implementation with proper metadata tracking
   - Add tool discovery from agent cards
   - Add periodic discovery support

2. **Enhanced RemoteToolExecutor**:
   - Implement standardized tool call protocol
   - Add improved error handling
   - Add result conversion utilities

### Phase 1b: Protocol & Client Integration (Week 2)

1. **A2A Protocol Integration**:
   - Implement tool discovery protocol
   - Implement tool invocation protocol
   - Implement tool response protocol

2. **Client Integration**:
   - Add new client methods for tool operations
   - Extend skill discovery to support tools
   - Add error handling integration

### Phase 1c: Testing & Documentation (Week 3)

1. **Test Suite**:
   - Unit tests for tool registry and executor
   - Integration tests with mock agents
   - End-to-end tests with real A2A client

2. **CLI Extensions**:
   - Add CLI commands for tool discovery
   - Add CLI commands for tool execution
   - Add CLI commands for tool result processing

3. **Documentation**:
   - Update README.md with tool usage examples
   - Add documentation for tool protocol
   - Update schema overview with tool-specific information

## Testing Strategy

1. **Unit Tests**:
   - Test tool registry and executor with mocked dependencies
   - Test protocol serialization/deserialization
   - Test error handling and retries

2. **Integration Tests**:
   - Test discovery of tools from mock agents
   - Test execution of tools with mock responses
   - Test error handling with simulated failures

3. **End-to-End Tests**:
   - Set up test agents with real tools
   - Test discovery and execution across agents
   - Test streaming and cancellation

## Compatibility and Migration

The implementation will maintain backward compatibility:

1. **Feature Flag Gating**: All new functionality will be behind the existing `bidir-delegate` feature flag
2. **Progressive Enhancement**: Basic functionality without new features will continue to work
3. **Graceful Degradation**: Agents without tool support will be handled gracefully

## Conclusion

This design enhances the existing remote tool execution capabilities within the A2A test suite while maintaining compatibility with the A2A protocol. It provides a structured approach to tool discovery, invocation, and result processing, with clear implementation phases and testing strategies.

The implementation will be done incrementally, starting with the core infrastructure, then integrating with the protocol and client, and finally adding comprehensive testing and documentation.