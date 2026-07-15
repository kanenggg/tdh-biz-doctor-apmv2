# Consultation-rs Service

## Overview

Rust microservice for managing consultation sessions with Twilio integration using PostgreSQL stored procedures.

## Features

- ✅ PostgreSQL stored procedures for efficient data access
- ✅ Twilio video/voice/chat session creation
- ✅ JWT access token generation
- ✅ Authorization checks (patient/doctor)
- ✅ Time window validation
- ✅ Session data upsert pattern
- ✅ Structured logging with tracing

## Database Setup

### Create Tables

```sql
-- Tables are in /Users/peelz/Workspace/doctor-apm/tdh-biz-doctor-apmv2/db/postgres/0002_new_biz_apm.sql
```

### Create Functions

```sql
-- Functions are in /Users/peelz/Workspace/doctor-apm/tdh-biz-doctor-apmv2/db/postgres/0003_funcs.sql
```

Functions:
- `get_consultation_session(booking_id, user_profile_id)` - Returns unified session info
- `upsert_session_info(booking_id, session_data)` - Inserts/updates session data

## Configuration

### Using .env File (Recommended)

```bash
# Copy example file
cp .env.example .env

# Edit with your actual values
nano .env  # or your preferred editor

# Run the service
cargo run
```

The `.env` file is automatically loaded on service startup. Note: `.env` is listed in `.gitignore` to prevent committing secrets.

### Environment Variables (APP__ prefix)

You can also set environment variables directly:

```bash
export APP__SERVER__HOST=0.0.0.0
export APP__SERVER__PORT=8080
export APP__DATABASE__USER=postgres_user
export APP__DATABASE__PASSWORD=your_password
export APP__DATABASE__HOST=localhost
export APP__DATABASE__PORT=5432
export APP__DATABASE__DATABASE_NAME=consultation
export APP__TWILIO__ACCOUNT_SID=AC...
export APP__TWILIO__API_KEY_SID=SK...
export APP__TWILIO__API_KEY_SECRET=...
export APP__TWILIO__AUTH_TOKEN=...
export APP__TWILIO__BASE_URL=https://video.twilio.com
export APP__SESSION__LEAD_TIME_SECONDS=300
export APP__SESSION__CONSULTATION_DURATION_MINUTES=30
```

### Config Files (Optional)

For more complex configurations, you can use TOML config files:

```bash
cp config/default.toml.example config/default.toml
cp config/local.toml.example config/local.toml
```

**Note:** Database configuration uses separate fields (user, password, host, port, database_name) instead of a single URL. This allows for more flexible configuration and environment variable overrides.

Config files are loaded in this order (later files override earlier ones):
1. `config/default.toml`
2. `config/local.toml`
3. Environment variables (APP__ prefix)

## Build & Run

### Build

```bash
cargo build
```

### Run

```bash
cargo run
```

### Development with Hot Reload (bacon)

For faster development iteration, use `bacon` for hot reload:

```bash
# Install bacon if not already installed
cargo install bacon

# Run with hot reload (auto-rebuilds and restarts on file changes)
bacon

# Run specific job
bacon check        # Quick syntax/type check
bacon test         # Run tests with hot reload
bacon integration-test # Run ignored integration tests
bacon clippy       # Run linter
bacon build        # Full build

# Run with custom config directory
bacon -- -- --config-dir /path/to/config
```

Bacon watches `src/**/*.rs` and `Cargo.toml` files for changes and automatically rebuilds/restarts the service. Configuration is in `bacon.toml`.

## API Endpoints

### Health Check

```bash
GET /health
Response: "OK"
```

### Get/Create Doctor Session

```bash
GET /v2/doctor/consultation/*:booking_id
```

**Request Headers:**
```
tdh-sec-iam-user-identity: {"accountId":123456,"accountType":888,"userProfileId":789,"userMainProfileId":789,"tenantId":1,"oidcUserId":"user123"}
```

The header contains a JSON string with `UserIdentity` data. If the header is missing or invalid, a default `UserIdentity` is used.

**Note:** The route uses wildcard routing (`/*booking_id`) to capture the booking ID from the path.

**Success Response:**
```json
{
  "__type": "GetDoctorSessionInfo.SessionReady",
  "sessionInfo": {
    "__type": "ProviderSessionInfo.Twilio",
    "sessionName": "mordee_twilio_video_123",
    "sessionChatName": "mordee_twilio_chat_123",
    "sessionToken": "eyJhbGc..."
  },
  "sessionStartTime": 1640995200,
  "sessionEndTime": 1640998800,
  "isFacialVerified": true,
  "isRequiredPhoneVerification": true,
  "sessionChannel": "VIDEO"
}
```

**Error Responses:**
```json
{ "__type": "GetDoctorSessionInfo.SessionNotFound" }
{ "__type": "GetDoctorSessionInfo.Unauthorized" }
{ "__type": "GetDoctorSessionInfo.SessionIsNotReady" }
{ "__type": "GetDoctorSessionInfo.SessionIsFinished" }
```

## Session Flow

1. **Get Consultation Session** - Query PostgreSQL function `get_consultation_session()`
   - Joins `booked_slot`, `appointment`, `session_info` tables
   - Returns unified `ConsultationSession` model

2. **Authorization Check** - Verify user is patient or doctor for the booking

3. **Time Window Check** - Validate within acceptable time window (lead_time)

4. **Session Data Handling**:
   - **Exists**: Generate fresh access token for existing room
   - **Missing**: Create Twilio room → upsert session data → generate token

5. **Return** - Session info with access token and metadata

## Architecture

```
┌───────────────────────────────────┐
│  HTTP Layer (Axum)            │
│  - Auth middleware (extracts   │
│    tdh-sec-iam-user-identity│
│    header as UserIdentity)     │
│  - Route handlers             │
└──────────────┬────────────────────┘
               │
┌──────────────▼────────────────────┐
│  Service Layer                   │
│  GetOrCreateConsultSessionService  │
│                                │
│  ┌───────────────────────────┐   │
│  │ GetOrCreateSessionRepo  │   │
│  │ (PostgreSQL functions)    │   │
│  └───────────────────────────┘   │
│                                │
│  ┌───────────────────────────┐   │
│  │ TwilioClient            │   │
│  │ (Video/Chat)            │   │
│  └───────────────────────────┘   │
│                                │
│  ┌───────────────────────────┐   │
│  │ EventPublisher           │   │
│  │ (PubSub)                │   │
│  └───────────────────────────┘   │
└──────────────────────────────────┘
```

## Session Status Flow

```
RoomCreated → Started → Ended
```

## Project Structure

```
consultation-rs/
├── src/
│   ├── handlers/
│   │   ├── middleware.rs          # Auth middleware - extracts UserIdentity
│   │   │                           # from tdh-sec-iam-user-identity header
│   │   └── v2/
│   │       └── doctor/
│   │           └── consultation.rs
│   ├── repo/
│   │   ├── get_or_create_session.rs
│   │   └── models/
│   │       └── provider_session_info.rs
│   ├── services/
│   │   ├── consultation/
│   │   │   └── doctor/
│   │   │       └── get_or_create_consult_session.rs
│   │   ├── twilio/
│   │   │   └── client.rs
│   │   └── event/
│   │       └── mod.rs
│   ├── sys/
│   │   └── config.rs          # Configuration with separate database fields
│   └── main.rs
├── config/
│   ├── default.toml.example
│   └── local.toml.example
└── Cargo.toml
```

## Database Configuration

The `DatabaseConfig` struct uses separate fields for connection parameters:

```rust
pub struct DatabaseConfig {
    pub user: String,
    pub password: String,
    pub host: String,
    pub port: u16,
    pub database_name: String,
}
```

The `connection_url()` method constructs the PostgreSQL connection string:

```rust
impl DatabaseConfig {
    pub fn connection_url(&self) -> String {
        format!(
            "postgresql://{}:{}@{}:{}/{}",
            self.user, self.password, self.host, self.port, self.database_name
        )
    }
}
```

This allows for flexible configuration using either:
- **Environment variables** with separate fields
- **TOML config files** with structured sections
- **Database URLs** (if you prefer, add `url` field to config)

## Integration Testing

### Using IT HTTP Client Environment

For integration testing with the IT HTTP Client tool, the environment file is configured at:

**File:** `../../it/http/http-client.env.json`

This file adds the mock `user-identity-json` header to all requests:

```json
{
  "accountId": 123456,
  "accountType": 888,
  "userProfileId": 789,
  "userMainProfileId": 789,
  "tenantId": 1,
  "oidcUserId": "test-user"
}
```

**To use:**
```bash
# In IT HTTP Client, select the local environment
# The service will receive the user-identity-json header automatically
```

## Error Handling

- `SessionNotFound` - No consultation session found
- `SessionIsNotReady` - Outside acceptable time window
- `SessionIsFinished` - Session has ended
- `Unauthorized` - User not patient or doctor for booking
- `InvalidSessionStatus` - Invalid session status in database
- `TwilioError` - Twilio API errors
- `DatabaseError` - Database operation errors

## Dependencies

- `axum` - Web framework
- `sqlx` - PostgreSQL (runtime queries)
- `tokio` - Async runtime
- `common-rs` - Twilio integration
- `protocol-rs` - Shared protocol definitions
- `tracing` - Structured logging
- `anyhow` - Error handling
- `thiserror` - Error types
- `serde` - Serialization
- `serde_json` - JSON handling
- `chrono` - Date/time
- `jiff` - Timestamp utilities
- `config` - Configuration management

