-- Initial schema for admin_backend

-- Feature overrides synchronized from wx_db
CREATE TABLE IF NOT EXISTS card_feature_override (
    pronunciation TEXT PRIMARY KEY NOT NULL,
    fixed_bits1 INTEGER NOT NULL,
    fixed_bits2 INTEGER NOT NULL,
    fixed_burst_bits INTEGER NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    note TEXT
);

-- Feature confirmations (new functionality)
CREATE TABLE IF NOT EXISTS feature_confirmation (
    pronunciation TEXT PRIMARY KEY NOT NULL,
    confirmed_at TEXT NOT NULL DEFAULT (datetime('now')),
    confirmed_by TEXT NOT NULL,  -- client_id
    rule_version TEXT,
    feature_bits1 INTEGER NOT NULL,
    feature_bits2 INTEGER NOT NULL,
    burst_bits INTEGER NOT NULL
);

-- Rule patterns synchronized from wx_db
CREATE TABLE IF NOT EXISTS rule_pattern (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    keyword TEXT NOT NULL,
    pattern TEXT NOT NULL,
    feature_name TEXT NOT NULL,
    is_enabled INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Sync metadata for tracking synchronization history
CREATE TABLE IF NOT EXISTS sync_metadata (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    client_id TEXT NOT NULL,
    sync_type TEXT NOT NULL CHECK (sync_type IN ('push', 'pull')),
    data_type TEXT NOT NULL CHECK (data_type IN ('feature_override', 'rule_pattern', 'confirmed_feature')),
    items_count INTEGER NOT NULL,
    synced_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- API keys for authentication
CREATE TABLE IF NOT EXISTS api_keys (
    key_hash TEXT PRIMARY KEY NOT NULL,
    client_name TEXT NOT NULL UNIQUE,
    permissions TEXT NOT NULL CHECK (permissions IN ('read', 'read_write')),
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    last_used_at TEXT
);

-- Indexes for performance
CREATE INDEX IF NOT EXISTS idx_feature_override_updated_at ON card_feature_override(updated_at);
CREATE INDEX IF NOT EXISTS idx_rule_pattern_keyword ON rule_pattern(keyword);
CREATE INDEX IF NOT EXISTS idx_sync_metadata_client ON sync_metadata(client_id, synced_at);
CREATE INDEX IF NOT EXISTS idx_api_keys_client_name ON api_keys(client_name);