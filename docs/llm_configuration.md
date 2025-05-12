# LLM Configuration Guide

> Related Documentation:
> - [Project Overview](../README.md)
> - [Bidirectional Agent](../src/bidirectional/README.md)
> - [Bidirectional Agent Quickstart](../README_BIDIRECTIONAL.md)

This document provides information about the Large Language Model (LLM) configuration in the A2A Test Suite.

## Overview

The A2A Test Suite uses LLMs for several features:
- Task routing decisions (deciding whether to handle tasks locally or delegate to another agent)
- Task decomposition (breaking down complex tasks into subtasks)
- Result synthesis (combining results from multiple subtasks)

The consolidated LLM configuration allows you to control all these features from a single configuration section, with the ability to override settings for specific use cases.

## Configuration Structure

The LLM configuration is specified in the `[llm]` section of your agent configuration TOML file.

### Basic Configuration

```toml
[llm]
use_for_routing = true                 # Whether to use LLM for routing decisions
use_new_core = true                    # Whether to use the new LLM core infrastructure
provider = "claude"                    # LLM provider (currently only "claude" is supported)
api_key = "sk-ant-api03-..."           # API key (if not provided, will use environment variable)
api_key_env_var = "ANTHROPIC_API_KEY"  # Environment variable name for API key
model = "claude-3-haiku-20240307"      # Default model to use
api_base_url = "https://api.example.com" # Optional custom API base URL
max_tokens = 2048                      # Default maximum tokens to generate
temperature = 0.1                      # Default temperature (0.0 - 1.0)
timeout_seconds = 30                   # Request timeout in seconds
```

### Use-Case Specific Overrides

You can override settings for specific use cases (routing, decomposition, synthesis):

```toml
[llm.use_cases.routing]
model = "claude-3-haiku-20240307"      # Model override for routing
max_tokens = 1024                      # Max tokens override for routing
temperature = 0.2                      # Temperature override for routing
prompt_template = "..."                # Custom prompt template for routing

[llm.use_cases.decomposition]
model = "claude-3-sonnet-20240229"     # Model override for decomposition
max_tokens = 4096                      # Max tokens override for decomposition
temperature = 0.3                      # Temperature override for decomposition
prompt_template = "..."                # Custom prompt template for decomposition

[llm.use_cases.synthesis]
model = "claude-3-opus-20240229"       # Model override for synthesis
max_tokens = 8192                      # Max tokens override for synthesis
temperature = 0.4                      # Temperature override for synthesis
prompt_template = "..."                # Custom prompt template for synthesis
```

## Default Values

If not specified, the following default values are used:

| Field | Default Value |
|-------|---------------|
| use_for_routing | false |
| use_new_core | false |
| provider | "claude" |
| api_key | None (will try environment variable) |
| api_key_env_var | "ANTHROPIC_API_KEY" |
| model | "claude-3-haiku-20240307" |
| api_base_url | None (uses provider's default endpoint) |
| max_tokens | 2048 |
| temperature | 0.1 |
| timeout_seconds | 30 |

## API Key Configuration

There are two ways to provide the API key:

1. Directly in the configuration:
   ```toml
   [llm]
   api_key = "sk-ant-api03-your-key"
   ```

2. Through an environment variable (more secure, recommended for production):
   ```toml
   [llm]
   api_key_env_var = "ANTHROPIC_API_KEY"
   ```
   Then set the environment variable:
   ```sh
   export ANTHROPIC_API_KEY="sk-ant-api03-your-key"
   ```

## Prompt Templates

Custom prompt templates can be provided for each use case. The templates can include specific placeholders that will be replaced at runtime:

- Routing templates: 
  - `{task_description}` - The task text
  - `{tools_description}` - Description of available tools
  - `{agents_description}` - Description of available agents

- Decomposition templates:
  - `{task_description}` - The task text

- Synthesis templates:
  - `{subtask_results}` - Results from all subtasks

## Examples

### Minimal Configuration with API Key from Environment

```toml
[llm]
use_for_routing = true
```

### Basic Configuration with API Key

```toml
[llm]
use_for_routing = true
api_key = "sk-ant-api03-your-key"
model = "claude-3-haiku-20240307"
max_tokens = 2048
temperature = 0.1
```

### Complete Configuration with Use Case Overrides

```toml
[llm]
use_for_routing = true
use_new_core = true
provider = "claude"
api_key = "sk-ant-api03-your-key"
model = "claude-3-haiku-20240307"
max_tokens = 2048
temperature = 0.1
timeout_seconds = 30

[llm.use_cases.routing]
model = "claude-3-haiku-20240307"
max_tokens = 1024
temperature = 0.2

[llm.use_cases.decomposition]
model = "claude-3-sonnet-20240229"
max_tokens = 4096
temperature = 0.3

[llm.use_cases.synthesis]
model = "claude-3-opus-20240229"
max_tokens = 8192
temperature = 0.4
```

## Validation

The following validation is performed on the LLM configuration:

- If `use_for_routing` is true, an API key must be available (either in config or environment)
- `provider` must be "claude" (currently the only supported provider)
- `temperature` must be between 0.0 and 1.0
- `max_tokens` must be at least 1
- `timeout_seconds` must be at least 1
- Any use-case specific overrides for `temperature` and `max_tokens` are also validated