DEV_ARTIFACT_NAME := "asia-southeast1-docker.pkg.dev/tdg-dh-truehealth-core-nonprod/cossack-docker"
PROJECT := "tdg-dh-truehealth-core-nonprod"
REGION := "asia-southeast1"
REGISTRY := "asia-southeast1-docker.pkg.dev"
REPO := "tdg-dh-truehealth-core-nonprod/cossack-docker"
TAG := `git rev-parse --short HEAD`

default:
    @just --list

[arg("platform", long)]
build-local module_name tag=TAG builder="docker" platform="linux/amd64":
    {{builder}} build \
      --ssh default \
      --platform {{platform}} \
      --build-arg MODULE_NAME={{module_name}} \
      -f rust.Dockerfile \
      -t {{DEV_ARTIFACT_NAME}}/tdh-biz-apm-{{module_name}}:{{tag}} \
      .

build-gcloud-docker module_name tag=TAG:
    echo "Building {{module_name}} {{tag}}"
    gcloud builds submit \
      --config=rust.cloudbuild.yaml \
      --substitutions=_MODULE_NAME={{module_name}},_TAG={{tag}},_IMAGE={{DEV_ARTIFACT_NAME}}/tdh-biz-apm-{{module_name}} \
      --project={{PROJECT}} \
      --region={{REGION}} 


build-buildpack module_name tag=TAG platform="linux/amd64":
    gcloud builds submit \
      --pack \
      --builder=gcr.io/buildpacks/builder:v1 \
      --env GOOGLE_ENTRYPOINT="./application" \
      --env GO_BUILD=true \
      --env GOPROXY=direct \
      --tag {{DEV_ARTIFACT_NAME}}/tdh-biz-apm-{{module_name}}:{{tag}} \
      --substitutions=_MODULE_NAME={{module_name}},_TAG={{tag}} \
      --project={{PROJECT}} \
      --region={{REGION}}

# Build CLI tool
build-cli:
  cargo build -p cli --release

# Generate OpenAPI specification (JSON + YAML)
openapi module:
  cargo build -p cli --bin openapi --release
  ./target/release/openapi generate --module {{module}}

# Generate all OpenAPI specifications (JSON + YAML)
openapi-all:
  cargo build -p cli --bin openapi --release
  ./target/release/openapi generate-all

# Export producer-owned OpenAPI specs
export-openapi:
  cargo build -p cli --bin openapi --release
  ./target/release/openapi generate-all
  @echo "OpenAPI specs exported to ./specs/provides/"
  @ls -la ./specs/provides/

# Create database migration
add-migration db_name migration_name:
  touch db/{{db_name}}/migrations/$(date +%Y%m%d%H%M%S)__{{migration_name}}.sql

