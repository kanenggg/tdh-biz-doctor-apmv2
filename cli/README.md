# CLI Tools

This directory contains command-line tools for the project.

## twilio-cli

Generate Twilio JWT tokens for video, chat, or combined access.

### Installation

```bash
cargo install --path .
```

### Usage

#### Video Token

```bash
# Using CLI flags
twilio video --account-sid <ACxxx> --api-key-sid <SKxxx> --api-key-secret <secret> --room-name <room> --identity <user>

# Using config file (config file has highest priority)
twilio video --config config.toml

# Config file option can be specified before or after the subcommand
twilio video --config config.toml
# or
twilio video --config config.toml video
```

#### Chat Token

```bash
# Using CLI flags
twilio chat --account-sid <ACxxx> --api-key-sid <SKxxx> --api-key-secret <secret> --service-sid <ISxxx> --identity <user>

# Using config file
twilio chat --config config.toml
```

#### Video+Chat Token

```bash
# Using CLI flags
twilio video-chat --account-sid <ACxxx> --api-key-sid <SKxxx> --api-key-secret <secret> --room-name <room> [--service-sid <ISxxx>] --identity <user>

# Using config file
twilio video-chat --config config.toml
```

### Config File Priority

The config file has **highest priority**:
1. Config file values are used if defined
2. CLI flags are used as fallback if config values are missing

Example:
```bash
# If config.toml contains: account_sid = "AC_CONFIG"
twilio video --config config.toml --account-sid AC_CLI
# Result: AC_CONFIG (from config file, CLI flag ignored)
```

### Config File Format

The config file uses TOML format.

#### Video Config (config.example.toml)

```toml
account_sid = "AC1234567890abcdef"
api_key_sid = "SK1234567890abcdef"
api_key_secret = "your_secret_key_here"
room_name = "my_video_room"
identity = "user_123"
```

#### Chat Config (config-chat.example.toml)

```toml
account_sid = "AC1234567890abcdef"
api_key_sid = "SK1234567890abcdef"
api_key_secret = "your_secret_key_here"
service_sid = "IS1234567890abcdef"
identity = "user_123"
```

#### Video+Chat Config (config-video-chat.example.toml)

```toml
account_sid = "AC1234567890abcdef"
api_key_sid = "SK1234567890abcdef"
api_key_secret = "your_secret_key_here"
room_name = "my_video_room"
service_sid = "IS1234567890abcdef"
identity = "user_123"
```

### Examples

```bash
# Generate video token using config file
twilio video --config config.example.toml

# Generate chat token using config file
twilio chat --config config-chat.example.toml

# Generate video+chat token using config file
twilio video-chat --config config-video-chat.example.toml

# Mix config file and CLI flags (config takes priority)
twilio video --config config.example.toml --identity override_user
# Note: if identity is also in config file, config value will be used
```

## openapi-cli

Generate OpenAPI specifications for services.

### Installation

```bash
cargo build --bin openapi
```

### Usage

#### Generate OpenAPI spec for consultation-rs

```bash
./target/debug/openapi consultation -o consultation-openapi.json
```

#### Generate OpenAPI spec for doctor-pool

```bash
./target/debug/openapi doctor-pool -o doctor-pool-openapi.json
```

#### Generate OpenAPI specs for all services

```bash
./target/debug/openapi all -o ./specs
```

This will create:
- `./specs/consultation-openapi.json`
- `./specs/doctor-pool-openapi.json`

### Options

- `consultation`: Generate OpenAPI spec for consultation-rs service
  - `-o, --output`: Output file path (default: `consultation-openapi.json`)
- `doctor-pool`: Generate OpenAPI spec for doctor-pool service
  - `-o, --output`: Output file path (default: `doctor-pool-openapi.json`)
- `all`: Generate OpenAPI specs for all services
  - `-o, --output-dir`: Output directory (default: `.`)

### Examples

```bash
# Generate consultation OpenAPI spec
./target/debug/openapi consultation -o consultation-openapi.json

# Generate doctor-pool OpenAPI spec
./target/debug/openapi doctor-pool -o doctor-pool-openapi.json

# Generate all specs to a specific directory
./target/debug/openapi all -o ./openapi-specs

# Generate all specs to current directory
./target/debug/openapi all
```

## Accessing API Documentation

Both consultation-rs and doctor-pool services include built-in Swagger UI accessible at:

### consultation-rs
```bash
cd consultation-rs
cargo run
```
Visit: http://localhost:3000/api-docs

### doctor-pool
```bash
cd doctor-pool
cargo run
```
Visit: http://localhost:3000/api-docs

The Swagger UI provides:
- Interactive API documentation
- Try out API endpoints directly from browser
- View request/response schemas
- Download OpenAPI spec as JSON at `/api-docs/openapi.json`


### Usage

#### Video Token

```bash
# Using CLI flags
twilio video --account-sid <ACxxx> --api-key-sid <SKxxx> --api-key-secret <secret> --room-name <room> --identity <user>

# Using config file (config file has highest priority)
twilio video --config config.toml

# Config file option can be specified before or after the subcommand
twilio video --config config.toml
# or
twilio --config config.toml video
```

#### Chat Token

```bash
# Using CLI flags
twilio chat --account-sid <ACxxx> --api-key-sid <SKxxx> --api-key-secret <secret> --service-sid <ISxxx> --identity <user>

# Using config file
twilio chat --config config.toml
```

#### Video+Chat Token

```bash
# Using CLI flags
twilio video-chat --account-sid <ACxxx> --api-key-sid <SKxxx> --api-key-secret <secret> --room-name <room> [--service-sid <ISxxx>] --identity <user>

# Using config file
twilio video-chat --config config.toml
```

### Config File Priority

The config file has **highest priority**:
1. Config file values are used if defined
2. CLI flags are used as fallback if config values are missing

Example:
```bash
# If config.toml contains: account_sid = "AC_CONFIG"
twilio video --config config.toml --account-sid AC_CLI
# Result: AC_CONFIG (from config file, CLI flag ignored)
```

### Config File Format

The config file uses TOML format.

#### Video Config (config.example.toml)

```toml
account_sid = "AC1234567890abcdef"
api_key_sid = "SK1234567890abcdef"
api_key_secret = "your_secret_key_here"
room_name = "my_video_room"
identity = "user_123"
```

#### Chat Config (config-chat.example.toml)

```toml
account_sid = "AC1234567890abcdef"
api_key_sid = "SK1234567890abcdef"
api_key_secret = "your_secret_key_here"
service_sid = "IS1234567890abcdef"
identity = "user_123"
```

#### Video+Chat Config (config-video-chat.example.toml)

```toml
account_sid = "AC1234567890abcdef"
api_key_sid = "SK1234567890abcdef"
api_key_secret = "your_secret_key_here"
room_name = "my_video_room"
service_sid = "IS1234567890abcdef"
identity = "user_123"
```

### Examples

```bash
# Generate video token using config file
twilio video --config config.example.toml

# Generate chat token using config file
twilio chat --config config-chat.example.toml

# Generate video+chat token using config file
twilio video-chat --config config-video-chat.example.toml

# Mix config file and CLI flags (config takes priority)
twilio video --config config.example.toml --identity override_user
# Note: if identity is also in config file, config value will be used
```

## openapi-cli

Generate OpenAPI specifications for services.

### Installation

```bash
cargo build --bin openapi
```

### Usage

#### Generate OpenAPI spec for consultation-rs

```bash
./target/debug/openapi consultation -o consultation-openapi.json
```

#### Generate OpenAPI spec for doctor-pool

```bash
./target/debug/openapi doctor-pool -o doctor-pool-openapi.json
```

#### Generate OpenAPI specs for all services

```bash
./target/debug/openapi all -o ./specs
```

This will create:
- `./specs/consultation-openapi.json`
- `./specs/doctor-pool-openapi.json`

### Options

- `consultation`: Generate OpenAPI spec for consultation-rs service
  - `-o, --output`: Output file path (default: `consultation-openapi.json`)
- `doctor-pool`: Generate OpenAPI spec for doctor-pool service
  - `-o, --output`: Output file path (default: `doctor-pool-openapi.json`)
- `all`: Generate OpenAPI specs for all services
  - `-o, --output-dir`: Output directory (default: `.`)

### Examples

```bash
# Generate consultation OpenAPI spec
./target/debug/openapi consultation -o consultation-openapi.json

# Generate doctor-pool OpenAPI spec
./target/debug/openapi doctor-pool -o doctor-pool-openapi.json

# Generate all specs to a specific directory
./target/debug/openapi all -o ./openapi-specs

# Generate all specs to current directory
./target/debug/openapi all
```

## Accessing API Documentation

Both consultation-rs and doctor-pool services include built-in Swagger UI accessible at:

### consultation-rs
```bash
cd consultation-rs
cargo run
```
Visit: http://localhost:3000/api-docs

### doctor-pool
```bash
cd doctor-pool
cargo run
```
Visit: http://localhost:3000/api-docs

The Swagger UI provides:
- Interactive API documentation
- Try out API endpoints directly from the browser
- View request/response schemas
- Download OpenAPI spec as JSON at `/api-docs/openapi.json`

