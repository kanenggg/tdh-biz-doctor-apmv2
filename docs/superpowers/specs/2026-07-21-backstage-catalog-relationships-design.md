# Backstage Catalog Relationships Design

## Goal

Update `catalog-info.yaml` so that the repository's active deployable services,
API contracts, and infrastructure resources are represented as separate
Backstage entities connected through explicit catalog relations.

## Scope

- Represent `consultation-rs` and `consultation-bg-rs` as separate Component
  entities.
- Place all first-party entities in one `biz-apm` System.
- Link `consultation-rs` to the OpenAPI contract it provides.
- Link components to checked-in API or event contracts only where the producer
  or consumer relationship is confirmed by source code and repository specs.
- Link components to PostgreSQL, Redis, Pub/Sub, KMS, and Cloud Storage resources
  only where their runtime use is confirmed by source or configuration.
- Use the repository's actual GitHub project slug for both components.

## Exclusions

- Do not add the Prescription API supplied as conversational context because no
  direct runtime consumption is established in this repository.
- Do not add removed packages such as `doctor-pool`, `twilio-rs`, or `report`.
- Do not add Kubernetes, Argo CD, SonarQube, or TechDocs annotations without
  confirmed identifiers and configuration.
- Do not invent deployment links or external resource ownership.

## Entity Model

The catalog will contain:

- `System`: `biz-apm`.
- `Component`: `consultation-rs`, providing its HTTP API and depending on its
  confirmed data and Google Cloud resources.
- `Component`: `consultation-bg-rs`, consuming confirmed inbound event contracts,
  publishing the checked-in consultation event contract, and depending on its
  confirmed PostgreSQL and Pub/Sub resources.
- `API` entities backed by files under `specs/provides` and `specs/depends-on`.
- Shared `Resource` entities for PostgreSQL, Redis, Pub/Sub, KMS, and Cloud
  Storage where applicable.

Relations will use Backstage's standard `system`, `providesApis`, `consumesApis`,
and `dependsOn` fields. No parent repository Component is needed because it would
not represent a deployable runtime and would add an artificial hierarchy.

## Validation

- Parse the multi-document file as YAML.
- Confirm every relation target resolves to an entity declared in the same
  catalog file.
- Scan for placeholders and stale doctor-app paths or entity names.
- Review the final diff to ensure unrelated files are unchanged.
