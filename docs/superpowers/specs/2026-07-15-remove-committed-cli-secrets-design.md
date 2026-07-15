# Remove Committed CLI Secrets

## Goal

Make the initial commit safe to push by removing developer-local Twilio configuration from Git history while preserving the local files for continued use.

## Changes

- Stop tracking `cli/config.toml` and `cli/config.local.yaml` without deleting their working-tree copies.
- Add both paths to a root `.gitignore`.
- Keep the existing `cli/config*.example.toml` files tracked as configuration templates.
- Correct `catalog-info.yaml` to target `./specs/provides/consultation-rs.yaml`.
- Amend the existing initial commit so the sensitive files do not remain in the history being pushed.

## Safety Constraints

- Never print credential values.
- Do not delete either local configuration file.
- Do not push automatically.
- Do not modify unrelated user files.
- The user must rotate or revoke any real Twilio credentials independently because removing them from Git does not invalidate them.

## Verification

- Confirm both local configuration files still exist but are ignored and untracked.
- Confirm neither path exists in `HEAD`.
- Confirm `catalog-info.yaml` references an existing file.
- Scan `HEAD` for common private-key and provider-token signatures without printing matched values.
- Show the final commit and working-tree status; leave the push to the user.
