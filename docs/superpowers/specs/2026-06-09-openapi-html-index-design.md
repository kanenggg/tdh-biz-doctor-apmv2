# OpenAPI HTML Index Design

## Goal

Enhance the local OpenAPI CLI build so `just openapi consultation-rs` and `just openapi-all` produce a browsable HTML entry point at `openapi/index.html` beside the generated JSON and YAML specs.

## Current Context

The active OpenAPI generator is the `openapi` binary in `cli/src/openapi.rs`. It currently supports the active `consultation-rs` module and writes:

- `openapi/consultation-rs.json`
- `openapi/consultation-rs.yaml`

The runtime service also has `consultation-rs/openapi/swagger.html` and `consultation-rs/openapi/redoc.html`, but those templates point at service routes and are not suitable as checked-in local build output.

Stale docs and generated files still mention removed modules such as `doctor-pool`; this enhancement will not restore or target those modules.

## Design

The CLI will continue generating JSON and YAML first. After successful generation, it will also write `index.html` into the selected output directory.

The HTML page will be a local static viewer shell that references `./consultation-rs.json`. It will use CDN-hosted Swagger UI assets, which keeps the generated file small and avoids adding JavaScript or CSS assets to the repository. The page is intended for local inspection of the generated spec, not for production service routing.

For `generate --module consultation-rs`, the index page will show the generated `consultation-rs` spec. For `generate-all`, the page will be generated after all active modules complete successfully. The active module list remains `consultation-rs`.

## CLI Behavior

- `openapi generate --module consultation-rs --output-dir ./openapi` writes JSON, YAML, and `index.html`.
- `openapi generate-all --output-dir ./openapi` writes JSON, YAML, and `index.html`.
- Unsupported modules continue to return an error.
- `generate-all` should return an error if any module fails instead of logging and continuing silently.
- Status messages may continue going to stdout, matching the existing CLI style.

## File Ownership

- Modify `cli/src/openapi.rs` for generation behavior.
- Keep the generated local viewer at `openapi/index.html`.
- Do not hand-edit `openapi/consultation-rs.json` or `openapi/consultation-rs.yaml`; regenerate them through the CLI if implementation changes require it.
- Do not change `consultation-rs` runtime docs routes.

## Testing And Verification

Verification should prove the CLI builds and writes the expected files:

- `cargo build -p cli --bin openapi`
- `./target/debug/openapi generate-all`
- Confirm `openapi/index.html` exists.
- Confirm `openapi/index.html` references `consultation-rs.json`.

If helper functions are split into testable units, add focused unit tests for generated HTML content and failure behavior. If not, keep verification command based.
