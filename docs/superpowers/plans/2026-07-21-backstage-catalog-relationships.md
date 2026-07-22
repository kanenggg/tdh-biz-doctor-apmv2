# Backstage Catalog Relationships Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the repository-level Backstage entry with linked entities for the two active Rust services, their confirmed contracts, and their confirmed infrastructure dependencies.

**Architecture:** Model `consultation-rs` and `consultation-bg-rs` as separate deployable Components inside the `biz-apm` System. Connect them only to contracts and resources confirmed by checked-in runtime code, configuration, and active specs; omit the draft Prescription API and ambiguous Walrus draft contracts.

**Tech Stack:** Backstage Catalog descriptor YAML, OpenAPI 3.1, AsyncAPI 3.1, Ruby YAML parser for structural validation.

## Global Constraints

- Use `kanenggg/tdh-biz-doctor-apmv2` as the GitHub project slug.
- Use `user:default/p-bank` as owner, preserving the current catalog ownership.
- Do not add removed packages or unconfirmed Kubernetes, Argo CD, SonarQube, TechDocs, Prescription API, or deployment metadata.
- The active runtime event publication contract is `specs/provides/biz-apm-published-events.asyncapi.yaml`; the V2 AsyncAPI artifact is draft/model-only and must not be linked as a runtime contract.
- Every relation target must be declared in `catalog-info.yaml`.

---

### Task 1: Replace the catalog with linked runtime entities

**Files:**
- Modify: `catalog-info.yaml`

**Interfaces:**
- Consumes: active runtime structure in `consultation-rs`, `consultation-bg-rs`, and checked-in specifications under `specs/`.
- Produces: Backstage entities `component:default/consultation-rs`, `component:default/consultation-bg-rs`, `api:default/consultation-rs-api`, `api:default/biz-apm-published-events`, `api:default/doctor-profile-events`, and shared resources in `system:default/biz-apm`.

- [ ] **Step 1: Record the current catalog validation baseline**

Run:

```bash
ruby -e 'require "yaml"; puts YAML.load_stream(File.read("catalog-info.yaml")).length'
```

Expected: `4`, confirming the existing file parses as four YAML documents.

- [ ] **Step 2: Replace `catalog-info.yaml` with the linked catalog model**

Use this exact entity structure:

```yaml
apiVersion: backstage.io/v1alpha1
kind: Component
metadata:
  name: consultation-rs
  title: Biz APM Consultation API
  description: >-
    Axum API service for appointment booking, consultation sessions, doctor
    availability, patient verification, facial uploads, and summary notes.
  annotations:
    github.com/project-slug: kanenggg/tdh-biz-doctor-apmv2
  tags:
    - rust
    - axum
    - telehealth
    - consultation
spec:
  type: service
  lifecycle: production
  owner: user:default/p-bank
  system: biz-apm
  providesApis:
    - consultation-rs-api
    - biz-apm-published-events
  dependsOn:
    - resource:default/biz-apm-postgresql
    - resource:default/biz-apm-redis
    - resource:default/biz-apm-pubsub
    - resource:default/biz-apm-kms
    - resource:default/biz-apm-cloud-storage

---
apiVersion: backstage.io/v1alpha1
kind: Component
metadata:
  name: consultation-bg-rs
  title: Biz APM Consultation Background Service
  description: >-
    Background webhook and worker service for payment confirmation, appointment
    hold expiry, doctor profile projection, and transactional event publishing.
  annotations:
    github.com/project-slug: kanenggg/tdh-biz-doctor-apmv2
  tags:
    - rust
    - axum
    - worker
    - gcp-pubsub
spec:
  type: service
  lifecycle: production
  owner: user:default/p-bank
  system: biz-apm
  providesApis:
    - biz-apm-published-events
  consumesApis:
    - doctor-profile-events
  dependsOn:
    - resource:default/biz-apm-postgresql
    - resource:default/biz-apm-pubsub

---
apiVersion: backstage.io/v1alpha1
kind: API
metadata:
  name: consultation-rs-api
  title: Biz APM Consultation API
  description: HTTP API exposed by consultation-rs.
  tags:
    - rest
    - openapi
    - consultation
spec:
  type: openapi
  lifecycle: production
  owner: user:default/p-bank
  system: biz-apm
  definition:
    $text: ./specs/provides/consultation-rs.yaml

---
apiVersion: backstage.io/v1alpha1
kind: API
metadata:
  name: biz-apm-published-events
  title: Biz APM Published Events
  description: Runtime Pub/Sub events published by the Biz APM services.
  tags:
    - asyncapi
    - gcp-pubsub
    - consultation
spec:
  type: asyncapi
  lifecycle: production
  owner: user:default/p-bank
  system: biz-apm
  definition:
    $text: ./specs/provides/biz-apm-published-events.asyncapi.yaml

---
apiVersion: backstage.io/v1alpha1
kind: API
metadata:
  name: doctor-profile-events
  title: Doctor Profile Events
  description: Doctor profile events consumed by consultation-bg-rs.
  tags:
    - asyncapi
    - gcp-pubsub
    - doctor-profile
spec:
  type: asyncapi
  lifecycle: production
  owner: user:default/p-bank
  system: biz-apm
  definition:
    $text: ./specs/depends-on/doctor-profile-approved.asyncapi.yaml

---
apiVersion: backstage.io/v1alpha1
kind: Resource
metadata:
  name: biz-apm-postgresql
  description: PostgreSQL data store shared by the Biz APM services.
spec:
  type: database
  lifecycle: production
  owner: user:default/p-bank
  system: biz-apm

---
apiVersion: backstage.io/v1alpha1
kind: Resource
metadata:
  name: biz-apm-redis
  description: Redis cache used by consultation-rs.
spec:
  type: cache
  lifecycle: production
  owner: user:default/p-bank
  system: biz-apm

---
apiVersion: backstage.io/v1alpha1
kind: Resource
metadata:
  name: biz-apm-pubsub
  description: Google Cloud Pub/Sub topics and subscriptions used by the Biz APM services.
spec:
  type: message-broker
  lifecycle: production
  owner: user:default/p-bank
  system: biz-apm

---
apiVersion: backstage.io/v1alpha1
kind: Resource
metadata:
  name: biz-apm-kms
  description: Google Cloud KMS keys used to encrypt and decrypt clinical data.
spec:
  type: encryption-key
  lifecycle: production
  owner: user:default/p-bank
  system: biz-apm

---
apiVersion: backstage.io/v1alpha1
kind: Resource
metadata:
  name: biz-apm-cloud-storage
  description: Google Cloud Storage used for consultation facial uploads.
spec:
  type: object-storage
  lifecycle: production
  owner: user:default/p-bank
  system: biz-apm

---
apiVersion: backstage.io/v1alpha1
kind: System
metadata:
  name: biz-apm
  title: Biz APM
  description: >-
    Telehealth appointment and consultation system for booking, sessions,
    doctor availability, clinical summaries, and background event processing.
  tags:
    - telehealth
    - consultation
spec:
  owner: user:default/p-bank
```

- [ ] **Step 3: Validate YAML structure and document count**

Run:

```bash
ruby -e 'require "yaml"; docs = YAML.load_stream(File.read("catalog-info.yaml")); abort "expected 11 entities" unless docs.length == 11; puts docs.map { |d| "#{d.fetch("kind")}:#{d.fetch("metadata").fetch("name")}" }'
```

Expected: eleven lines containing two Components, three APIs, five Resources, and one System.

- [ ] **Step 4: Validate that every catalog relation resolves locally**

Run:

```bash
ruby -e 'require "yaml"; docs = YAML.load_stream(File.read("catalog-info.yaml")); ids = docs.map { |d| [d.fetch("kind").downcase, d.fetch("metadata").fetch("namespace", "default"), d.fetch("metadata").fetch("name")].join(":") }; normalize = ->(kind, ref) { parts=ref.split("/", 2); parts.length == 2 ? "#{kind}:#{parts[0]}:#{parts[1]}" : "#{kind}:default:#{ref}" }; refs = docs.flat_map { |d| s=d.fetch("spec", {}); Array(s["providesApis"]).map { |x| normalize.call("api", x) } + Array(s["consumesApis"]).map { |x| normalize.call("api", x) } + Array(s["dependsOn"]).map { |x| kind, name=x.split(":", 2); normalize.call(kind, name) } + (s["system"] ? [normalize.call("system", s["system"])] : []) }; missing=refs.uniq-ids; abort "missing relations: #{missing.join(", ")}" unless missing.empty?; puts "all relations resolve"'
```

Expected: `all relations resolve`.

- [ ] **Step 5: Check for excluded or stale catalog references**

Run:

```bash
rg -n 'Prescription|consultation-event-v2|tdh-mordee-doctor|doctor-pool|twilio-rs|server/openapi|<[^>]+>' catalog-info.yaml
```

Expected: no matches and exit status `1`.

- [ ] **Step 6: Review formatting and commit the catalog change**

Run:

```bash
git diff --check -- catalog-info.yaml
git diff -- catalog-info.yaml
git add catalog-info.yaml
git commit -m "docs: link backstage catalog entities"
```

Expected: `git diff --check` produces no output, the diff changes only `catalog-info.yaml`, and the commit succeeds.
