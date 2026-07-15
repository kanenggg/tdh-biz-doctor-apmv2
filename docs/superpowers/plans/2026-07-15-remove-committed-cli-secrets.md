# Remove Committed CLI Secrets Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove developer-local Twilio credentials from the Git history being pushed while preserving the repository's existing file and directory structure.

**Architecture:** Keep both local CLI config files on disk but make Git ignore them and remove them from the index. Correct the Backstage location reference, then fold all cleanup into a rewritten initial commit so the push contains no earlier secret-bearing commit.

**Tech Stack:** Git, `.gitignore`, Backstage catalog YAML

## Global Constraints

- Never print credential values.
- Do not delete `cli/config.toml` or `cli/config.local.yaml` from the working tree.
- Do not push automatically.
- Do not modify unrelated user files.
- Preserve the current directory structure.
- The user must rotate or revoke any real Twilio credentials independently.

---

### Task 1: Ignore Local CLI Configuration and Correct the Catalog Target

**Files:**
- Create: `.gitignore`
- Modify: `catalog-info.yaml`
- Preserve locally: `cli/config.toml`
- Preserve locally: `cli/config.local.yaml`

**Interfaces:**
- Consumes: existing local CLI configuration and `specs/provides/consultation-rs.yaml`
- Produces: Git ignore rules and a valid Backstage file target

- [ ] **Step 1: Record the precondition**

Run: `test -f cli/config.toml && test -f cli/config.local.yaml && test -f specs/provides/consultation-rs.yaml`
Expected: exit code 0.

- [ ] **Step 2: Add exact ignore rules**

Create `.gitignore` with:

```gitignore
# Developer-local CLI credentials
/cli/config.toml
/cli/config.local.yaml
```

- [ ] **Step 3: Correct the catalog target**

Set the target in `catalog-info.yaml` to:

```yaml
spec:
  type: file
  targets:
    - ./specs/provides/consultation-rs.yaml
```

- [ ] **Step 4: Stop tracking the config files without deleting them**

Run: `git rm --cached cli/config.toml cli/config.local.yaml`
Expected: both paths are staged for removal and both files remain on disk.

### Task 2: Rewrite and Verify the Initial Commit

**Files:**
- Modify history: current `main` commits
- Include: `.gitignore`, `catalog-info.yaml`, approved spec and this plan

**Interfaces:**
- Consumes: staged cleanup from Task 1
- Produces: one safe initial commit ready for a user-controlled push

- [ ] **Step 1: Rewrite the branch into one initial commit**

Run: create an orphan branch from the cleaned working tree, commit it, and replace local `main`.
Expected: `main` contains exactly one commit and neither local config path exists in its tree.

- [ ] **Step 2: Verify local files and Git state**

Run: `test -f cli/config.toml && test -f cli/config.local.yaml && git check-ignore cli/config.toml cli/config.local.yaml`
Expected: exit code 0 and both paths reported as ignored.

- [ ] **Step 3: Verify the catalog reference**

Run: extract the target from `catalog-info.yaml` and confirm the referenced path exists.
Expected: `./specs/provides/consultation-rs.yaml` resolves to an existing file.

- [ ] **Step 4: Verify history and scan common secret signatures**

Run: confirm one commit, confirm the two local paths are absent from `HEAD`, and scan tracked files for common private-key/provider-token signatures without printing values.
Expected: one commit, no tracked local configs, and no matching tracked files.

- [ ] **Step 5: Leave push to the user**

Run: `git status --short --branch`
Expected: a clean `main` branch; no `git push` is executed.
