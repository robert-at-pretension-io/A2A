-- Initial schema for agent directory using SQLite

-- Stores information about discovered agents
CREATE TABLE IF NOT EXISTS agents (
    agent_id TEXT PRIMARY KEY NOT NULL, -- Unique name/ID of the agent (usually from AgentCard.name)
    url TEXT NOT NULL,                  -- Last known base URL of the agent's server
    status TEXT NOT NULL DEFAULT 'active' CHECK(status IN ('active', 'inactive')), -- Current status
    last_verified TEXT NOT NULL,        -- ISO8601 timestamp (UTC) of the last verification attempt (success or fail)
    consecutive_failures INTEGER NOT NULL DEFAULT 0, -- Count of consecutive failed verification attempts
    last_failure_code INTEGER,          -- HTTP status code (or internal code like -1 for network error) of the last failure
    next_probe_at TEXT NOT NULL,        -- ISO8601 timestamp (UTC) for the next scheduled verification check (implements backoff)
    card_json TEXT                      -- Full agent card stored as a JSON string (optional, can be updated)
);

-- Index on status for quickly finding active or inactive agents
CREATE INDEX IF NOT EXISTS idx_agents_status ON agents(status);

-- Index on next_probe_at for efficiently finding agents due for verification
CREATE INDEX IF NOT EXISTS idx_agents_next_probe ON agents(next_probe_at);
