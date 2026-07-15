# Biz Apm project

## Devspace

Run dev

```shell

devspace run-pipeline ${service}
#ex. to run timeslot service with devspace
devspace run-pipeline timeslot-dev
```

Build image (run on Dockerfile has been changed)
```shell
devspace run-pipeline build
```

## Development with Bacon (Hot Reload)

Bacon provides hot reload for Rust development. It automatically rebuilds and restarts services when files change.

```bash
# Install bacon if not already installed
cargo install bacon

# Run with default settings
bacon

# Run specific jobs
bacon check              # Quick syntax/type check
bacon check-all          # Check all targets
bacon clippy             # Run linter
bacon clippy-all         # Run linter with all features
bacon test               # Run tests
bacon integration-test   # Run ignored consultation-rs integration tests
bacon doc                # Generate documentation
bacon doc-open           # Generate and open documentation
bacon consultation-rs    # Run consultation-rs service with hot reload
bacon consultation-bg-rs # Run consultation-bg-rs service with hot reload
```

### Running consultation-rs with custom config directory

```bash
# Run consultation-rs with hot reload and custom config directory
bacon consultation-rs-config -- --config-dir /path/to/config
```

Configuration is in `bacon.toml` at the workspace root.