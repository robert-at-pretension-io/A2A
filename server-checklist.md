# A2A Server Implementation Checklist

This checklist will help you ensure your A2A server implementation is complete and compliant with the protocol specification.

## Core Requirements

- [ ] **Agent Card Endpoint**: `/.well-known/agent.json` returns a valid agent capabilities card
- [ ] **API Base Path**: `/api/v1` prefix for all API endpoints (unless specified otherwise)
- [ ] **Content Types**: Proper handling of `application/json` content type
- [ ] **Status Codes**: Appropriate HTTP status codes for different responses
- [ ] **Error Format**: Standardized error responses following A2A error schema

## Essential Endpoints

- [ ] **Create Task**: `POST /api/v1/task` accepts and processes task creation requests
- [ ] **Get Task**: `GET /api/v1/task/{id}` returns task status and details
- [ ] **Cancel Task**: `POST /api/v1/task/{id}/cancel` handles cancellation requests
- [ ] **Health Check**: `GET /health` returns service health status

## Optional Capabilities

- [ ] **Streaming**: `GET /api/v1/task/{id}/stream` supports Server-Sent Events for streaming updates
- [ ] **Push Notifications**: 
  - [ ] `POST /api/v1/task/{id}/push-notification` configures webhooks
  - [ ] `GET /api/v1/task/{id}/push-notification` retrieves webhook configuration

- [ ] **State History**:
  - [ ] `GET /api/v1/task/{id}/state-history` retrieves task state transitions
  - [ ] `GET /api/v1/task/{id}/state-metrics` provides metrics on state transitions

- [ ] **Task Batching**:
  - [ ] `POST /api/v1/batch` creates task batches
  - [ ] `GET /api/v1/batch/{id}` retrieves batch details
  - [ ] `GET /api/v1/batch/{id}/status` gets batch status
  - [ ] `POST /api/v1/batch/{id}/cancel` cancels entire batch

- [ ] **File Operations**:
  - [ ] `POST /api/v1/file` handles file uploads
  - [ ] `GET /api/v1/file/{id}` serves file downloads
  - [ ] `GET /api/v1/file` lists available files (optional task-id filtering)

- [ ] **Agent Skills**:
  - [ ] `GET /api/v1/skill` lists available skills
  - [ ] `GET /api/v1/skill/{id}` provides skill details
  - [ ] `POST /api/v1/skill/{id}/invoke` invokes a skill

## Authentication Support

- [ ] **Bearer Token**: Support for `Authorization: Bearer [token]` header
- [ ] **Alternative Auth Schemes**: Support for API key or other authentication as needed
- [ ] **Auth Reflection**: Authentication requirement reflected in agent card

## Testing Your Implementation

1. First, run basic compliance test:
   ```
   ./a2a-test-suite run-tests --url http://your-server-url
   ```

2. If your server implements non-core features, run with unofficial tests:
   ```
   ./a2a-test-suite run-tests --url http://your-server-url --run-unofficial
   ```

3. Fix any failures reported by the test suite

## Common Issues

- **Missing or malformed agent card**: The agent card must correctly report capabilities.
- **Incorrect task state transitions**: Task states must follow the A2A state machine.
- **Authentication issues**: If auth is required, make sure it's correctly implemented.
- **Streaming implementation problems**: If supporting streaming, make sure SSE format is correct.
- **Content type handling**: Ensure your server correctly processes and returns JSON content.

## Next Steps

After your server passes all tests:

1. Try integration with real A2A clients 
2. Load test to ensure stability under high traffic
3. Monitor for any protocol compliance issues in production
4. Contribute feedback to improve the A2A protocol specification!