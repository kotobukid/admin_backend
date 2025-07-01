# CLAUDE.md - Admin Backend

This file provides guidance to Claude Code (claude.ai/code) when working with the admin_backend project.

## Project Overview

Admin Backend is a gRPC-based synchronization hub for the WIXOSS Trading Card Game database (wx_db) development across multiple machines. It manages data synchronization, feature confirmation states, and ensures consistency across different development environments.

## Current Implementation Status

### ✅ Implemented Features
- **gRPC Server**: Running on port 50051 with TLS support
- **SQLite Database**: Connection and migration system
- **API Key Authentication**: Metadata-based authentication with permission levels
- **Core Services**:
  - `GetSyncStatus`: Health check and sync status retrieval
  - `PushFeatureOverrides`: Stream-based feature override upload
  - `PullFeatureOverrides`: Query-based feature override download
  - `ConfirmFeatures`: Feature confirmation recording

### 🚧 Not Yet Implemented
- `GetConfirmedFeatures`: Retrieve confirmed features
- `UnconfirmFeature`: Remove feature confirmation
- `PushRulePatterns`: Rule pattern synchronization
- `PullRulePatterns`: Rule pattern retrieval
- `RecordSync`: Sync metadata recording (skeleton only)
- API key generation CLI tool
- TLS certificate configuration

## Architecture

```
[Local wx_db #1] <--gRPC/TLS--> [Admin Backend (VPS)] <--gRPC/TLS--> [Local wx_db #2]
      |                               |                                    |
   PostgreSQL                      SQLite3                            PostgreSQL
```

### Technology Stack
- **Language**: Rust
- **Framework**: tonic (gRPC), tokio (async runtime)
- **Database**: SQLite3 (supports i64 for feature bits)
- **Protocol**: gRPC with TLS (Let's Encrypt certificates)
- **Authentication**: API Key via gRPC metadata

## Key Components

### 1. Data Models
- **CardFeatureOverride**: Manual corrections for card features (pronunciation-based)
- **RulePattern**: Regex patterns for feature detection
- **SyncMetadata**: Track synchronization history
- **FeatureConfirmation**: Mark features as "confirmed" to skip re-analysis

### 2. gRPC Services
```proto
service AdminSync {
    // Feature Override Management
    rpc PushFeatureOverrides(stream FeatureOverride) returns (PushResponse);
    rpc PullFeatureOverrides(PullRequest) returns (stream FeatureOverride);
    
    // Feature Confirmation
    rpc ConfirmFeatures(ConfirmRequest) returns (ConfirmResponse);
    rpc GetConfirmedFeatures(Empty) returns (stream ConfirmedFeature);
    
    // Rule Pattern Sync
    rpc SyncRulePatterns(stream RulePattern) returns (SyncResponse);
    
    // Metadata
    rpc GetSyncStatus(StatusRequest) returns (StatusResponse);
}
```

### 3. Database Schema (SQLite)
```sql
-- Feature overrides (from wx_db)
CREATE TABLE card_feature_override (
    pronunciation TEXT PRIMARY KEY,
    fixed_bits1 INTEGER,  -- i64 support
    fixed_bits2 INTEGER,
    fixed_burst_bits INTEGER,
    created_at TEXT,
    updated_at TEXT,
    note TEXT
);

-- Feature confirmations (new)
CREATE TABLE feature_confirmation (
    pronunciation TEXT PRIMARY KEY,
    confirmed_at TEXT NOT NULL,
    confirmed_by TEXT NOT NULL,
    rule_version TEXT,
    feature_bits1 INTEGER,
    feature_bits2 INTEGER,
    burst_bits INTEGER
);

-- Sync metadata
CREATE TABLE sync_metadata (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    client_id TEXT NOT NULL,
    last_sync_at TEXT NOT NULL,
    sync_type TEXT NOT NULL,
    items_count INTEGER
);

-- API keys
CREATE TABLE api_keys (
    key_hash TEXT PRIMARY KEY,
    client_name TEXT NOT NULL,
    permissions TEXT NOT NULL,  -- 'read' or 'read_write'
    created_at TEXT NOT NULL,
    last_used_at TEXT
);
```

## Security

### TLS Configuration
- Uses Let's Encrypt certificates via certbot
- Domain required (e.g., admin.example.com)
- Auto-renewal configured

### Authentication
- API Key required in gRPC metadata
- Keys stored as hashed values in SQLite
- Per-client permissions (read/read_write)

## Project Structure

```
admin_backend/
├── src/
│   ├── main.rs          # Entry point
│   ├── auth.rs          # API key authentication
│   ├── database.rs      # Database connection and migration
│   ├── server.rs        # gRPC service implementation
│   └── proto/           # Generated protobuf code (gitignored)
├── proto/
│   └── admin.proto      # gRPC service definitions
├── migrations/
│   └── 001_initial_schema.sql
├── data/                # SQLite database files (gitignored)
├── .sqlx/               # SQLx offline query cache (gitignored)
└── Cargo.toml
```

## Development Workflow

### Running the Server
```bash
# Development mode
RUST_LOG=info cargo run

# With custom database
DATABASE_URL=sqlite://custom.db cargo run

# Production build
cargo build --release
```

### Adding New Features
1. Update proto definitions
2. Run `cargo build` to generate Rust code
3. Implement service methods
4. Add SQLite migrations if needed
5. Update client-side sync logic in wx_db

### Database Migrations
```bash
# Create new migration
sqlx migrate add <migration_name>

# Run migrations
sqlx migrate run --database-url sqlite://data/admin.db

# Prepare offline queries
DATABASE_URL=sqlite://data/admin.db cargo sqlx prepare
```

### Testing
```bash
# Compile check
cargo check

# Unit tests
cargo test

# With test database
DATABASE_URL=sqlite://data/test.db cargo test

# gRPC testing (server must be running)
# Note: API key required for authentication
grpcurl -plaintext -H "api-key: YOUR_API_KEY" localhost:50051 list
```

## Important Design Decisions

### 1. SQLite over PostgreSQL
- Simpler deployment and backup (single file)
- Sufficient for synchronization workload
- Native i64 support for feature bits

### 2. gRPC over REST
- Type safety with protobuf
- Efficient binary encoding for feature bits
- Streaming support for large syncs

### 3. Pronunciation-based Feature Management
- Features are applied to all cards with same pronunciation
- Reduces manual work and ensures consistency
- Aligns with wx_db's existing model

### 4. Confirmation System
- Once confirmed, features won't be re-analyzed
- Tracks which rule version was used
- Allows rollback if needed

## Common Operations

### Start Server
```bash
# Basic start
cargo run

# With logging
RUST_LOG=info cargo run

# Production
./target/release/admin_backend
```

### Sync Feature Overrides
```rust
// Client side (wx_db)
let overrides = fetch_local_overrides().await?;
let response = client.push_feature_overrides(stream::iter(overrides)).await?;
```

### Confirm Features
```rust
// Mark features as confirmed after manual review
let request = ConfirmRequest {
    pronunciation: "カードメイ".to_string(),
    feature_bits1: 0x1234,
    feature_bits2: 0x5678,
    burst_bits: 0x90,
};
client.confirm_features(request).await?;
```

### Check Database Content
```bash
# View feature overrides
sqlite3 data/admin.db "SELECT * FROM card_feature_override;"

# View confirmed features
sqlite3 data/admin.db "SELECT * FROM feature_confirmation;"

# View API keys (hashed)
sqlite3 data/admin.db "SELECT client_name, permissions, created_at FROM api_keys;"
```

## Troubleshooting

### Connection Issues
- Check TLS certificate validity
- Verify API key in metadata
- Ensure port 50051 is open

### Sync Conflicts
- Later timestamp wins
- Manual resolution UI planned
- Check sync_metadata for history

## Future Enhancements
- Web UI for manual review
- Conflict resolution strategies
- Bulk import/export tools
- Real-time sync via streaming

## Related Projects
- `../wx_db` - wx_db project (separate from this main project)

