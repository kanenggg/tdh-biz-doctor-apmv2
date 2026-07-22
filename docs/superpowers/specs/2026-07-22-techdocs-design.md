# Biz APM TechDocs Design

**Date:** 2026-07-22

## Objective

Publish one curated Backstage TechDocs site for the
`tdh-biz-doctor-apmv2` repository. The site belongs to the
`component:default/consultation-rs` catalog entity and documents both the HTTP
service and the background service without duplicating content.

The first release uses Backstage's basic local build and local publisher setup.
Moving generation to CI and publishing to cloud storage is a later operational
change.

## Source and Ownership

Documentation remains in the repository beside the code. Existing Markdown
files keep their current paths so links from reviews, issues, and source files
continue to work.

Only `consultation-rs` receives the `backstage.io/techdocs-ref: dir:.`
annotation. `consultation-bg-rs` remains part of the same documentation site
through the navigation and content. It does not receive a second annotation,
which avoids building and presenting the same site twice.

The catalog owner `user:default/p-bank` remains responsible for the TechDocs
site.

## Documentation Structure

Add `mkdocs.yml` at the repository root and `docs/index.md` as the landing page.
The MkDocs configuration uses the Material theme and the `techdocs-core` plugin.
External fonts are disabled in the repository configuration so generation does
not depend on Google Fonts access.

The navigation exposes maintained operational and architectural documents:

1. Overview
   - Home (`docs/index.md`)
2. Architecture and domain
   - Appointment Flow (`docs/APPOINTMENT_FLOW.md`)
   - Consultation Events (`docs/CONSULTATION_EVENT.md`)
3. APIs and integration
   - DoctorApp Integration (`docs/DOCTORAPP_INTEGRATION.md`)
   - Star Gate Booking and Timeslot API
     (`docs/star-gate-booking-timeslot-api.md`)
   - Doctor Projection Rollout
     (`docs/doctor-service-config-projection-rollout.md`)
4. Architecture decisions
   - ADRs `0001` through `0007`, in numeric order

The `docs/plans/**` and `docs/superpowers/**` trees are intentionally excluded
from navigation because they contain implementation history, working notes, and
point-in-time plans rather than the current service contract. MkDocs strict
navigation is not used, so those Markdown files can remain in the documentation
directory without making the build fail merely because they are not listed.

## Backstage Runtime Configuration

The Backstage deployment must enable the TechDocs frontend and backend plugins
and use this initial configuration:

```yaml
techdocs:
  builder: local
  generator:
    runIn: local
  publisher:
    type: local
```

Because `generator.runIn` is `local`, the Backstage runtime image must provide
Python and `mkdocs-techdocs-core`. If the deployment already supports Docker
socket access, `generator.runIn: docker` is an acceptable deployment-only
substitution; it does not change this repository design.

The Backstage application is outside the current workspace. Repository changes
can be completed and verified independently, but the TechDocs page cannot render
until the deployment configuration and dependencies are present.

## Validation and Failure Handling

Repository validation covers the boundaries under repository control:

- Parse `mkdocs.yml` as YAML.
- Require `site_name`, `docs_dir: docs`, and the `techdocs-core` plugin.
- Require every local Markdown path named in `nav` to exist.
- Require exactly one active `backstage.io/techdocs-ref: dir:.` annotation, on
  `consultation-rs`.
- Run a real MkDocs/TechDocs build when `mkdocs-techdocs-core` is available in
  the validation environment.

A missing Backstage runtime dependency must fail deployment validation with a
clear dependency error. It must not be hidden by changing catalog metadata or
duplicating generated HTML in this repository.

## Testing

Add focused tests to the existing catalog CI test suite for the TechDocs
configuration and catalog annotation. Extend the shared CI runner configuration
only if needed to invoke those repository-specific checks without making the
byte-identical workflow diverge from DoctorApp.

Local verification includes:

1. Existing catalog CI unit tests.
2. TechDocs configuration and navigation tests.
3. YAML parsing for `catalog-info.yaml` and `mkdocs.yml`.
4. MkDocs build after installing the same TechDocs dependencies as the
   Backstage runtime. Dedicated tests validate curated navigation; strict mode
   is not used because intentionally unlisted planning documents produce
   warnings.
5. Existing catalog parsing and OpenAPI validation.

## Out of Scope

- A separate TechDocs site for `consultation-bg-rs`.
- Publishing generated HTML to Git.
- CI publishing to Google Cloud Storage, S3, or another object store.
- Editing the Backstage deployment repository, which is not present in this
  workspace.
- Rewriting or migrating existing Markdown documents.
- Publishing implementation plans and Superpowers artifacts in the curated
  navigation.
