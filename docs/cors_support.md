# CORS Support in A2A Test Suite

## Overview

The A2A Test Suite now includes Cross-Origin Resource Sharing (CORS) support, allowing web applications from different origins to interact with the A2A API endpoints. This is essential for browser-based clients to communicate with the A2A server.

## Implementation Details

CORS support has been added with the following features:

1. **Preflight Requests**: The server properly handles OPTIONS preflight requests, responding with appropriate CORS headers.

2. **Global CORS Headers**: All API responses include the necessary CORS headers:
   - `Access-Control-Allow-Origin: *` - Allows requests from any origin
   - `Access-Control-Allow-Methods: GET, POST, OPTIONS` - Allows common HTTP methods
   - `Access-Control-Allow-Headers: Content-Type, Authorization` - Allows essential headers
   - `Access-Control-Max-Age: 86400` - Caches preflight results for 24 hours

3. **Comprehensive Testing**: Includes dedicated CORS tests that verify header presence and proper handling of preflight requests.

## Usage

No special configuration is required to use CORS support. It's enabled by default for all server endpoints, including:

- Agent card retrieval (`/.well-known/agent.json`)
- JSON-RPC API endpoints
- Error responses

## Example Client Usage

Here's an example of how a browser-based client can interact with the A2A server:

```javascript
// Using fetch API from a web browser
async function getAgentCard(serverUrl) {
  try {
    const response = await fetch(`${serverUrl}/.well-known/agent.json`, {
      method: 'GET',
      headers: {
        'Accept': 'application/json'
      }
    });
    
    if (!response.ok) {
      throw new Error(`HTTP error! status: ${response.status}`);
    }
    
    const agentCard = await response.json();
    console.log('Agent card:', agentCard);
    return agentCard;
  } catch (error) {
    console.error('Error fetching agent card:', error);
    throw error;
  }
}

// Example JSON-RPC call
async function sendTask(serverUrl, message) {
  try {
    const response = await fetch(serverUrl, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json'
      },
      body: JSON.stringify({
        jsonrpc: '2.0',
        id: 1,
        method: 'tasks/send',
        params: {
          input: {
            role: 'user',
            content: [
              {
                type: 'text',
                text: message
              }
            ]
          }
        }
      })
    });
    
    if (!response.ok) {
      throw new Error(`HTTP error! status: ${response.status}`);
    }
    
    const result = await response.json();
    console.log('Task result:', result);
    return result;
  } catch (error) {
    console.error('Error sending task:', error);
    throw error;
  }
}
```

## Testing CORS Support

The A2A Test Suite includes automated tests for CORS functionality in `/src/server/tests/cors_test.rs`.

You can run these tests with:

```bash
RUSTFLAGS="-A warnings" cargo test server::tests::cors_test
```

The tests verify:
1. OPTIONS preflight requests return proper CORS headers
2. Agent card requests include CORS headers
3. Regular and error responses include CORS headers

## Security Considerations

The current implementation uses `Access-Control-Allow-Origin: *`, which allows requests from any origin. In production environments, you might want to restrict this to specific trusted origins.

To modify the allowed origins, update the `add_cors_headers` function in `/src/server/mod.rs` and `/src/server/handlers/mod.rs`.