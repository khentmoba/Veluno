---
name: discord-release
description: Package Veluno, publish GitHub release assets, and announce the Windows installer in Discord #veluno-releases. Use for releases or release announcements.
---

# Discord & GitHub Release Pipeline for Veluno

Builds and verifies Veluno, publishes GitHub release assets, and announces the
Windows installer in Discord `#veluno-releases`.

## Artifact selection

Discord distributes `target/release/veluno-setup.exe`. The notifier attaches it
when it exists and is at most 10 MiB. Otherwise, including `--no-upload`, it links
to `veluno-setup.exe` on the matching GitHub release. Verify that asset exists
before announcing a link, especially with `--skip-github` or a notification-only
request. Use `--installer <path>` for a custom installer location; `--exe` remains
a compatibility alias and must also receive an installer.

GitHub retains `veluno.exe`, `veluno-setup.exe`, and `SHA256SUMS`, plus identical
`felaris.exe` and `felaris-setup.exe` compatibility copies for installed legacy updaters.
The manifest uses the old repository redirect and `felaris.exe` filename; keep both
compatibility copies in `SHA256SUMS`. Keep the raw
`veluno.exe` asset and `version.json` download URL for the built-in auto-updater.
Discord's download and installation instructions use `veluno-setup.exe`.

## Quick Commands

| Action | Command |
|---|---|
| **Prepare release notes before committing** | `python scripts/build.py --prepare -s "Release summary"` |
| **Build & Publish Release (GitHub + Discord)** | `python scripts/build.py` (or `.\scripts\build.ps1`) |
| **Build & Publish Discord only (skip GitHub)** | `python scripts/build.py --skip-github` |
| **Build without Discord notification** | `python scripts/build.py --skip-discord` |
| **Notify/Upload existing build manually** | `python scripts/notify_discord.py -s "Changelog summary"` |
| **Test webhook connection** | `python scripts/notify_discord.py --test` |
| **Dry run (inspect payload without sending)** | `python scripts/build.py --dry-run` |

## Direct release workflow

A release request authorizes preparing and pushing the release metadata directly
to `main`, publishing the GitHub release, and sending its Discord announcement.
Create a release PR only when the user explicitly requests one. A notification-only
request uses the existing release without a version bump or rebuild.

1. Fetch `origin/main` and prepare the release from that commit, preserving unrelated
   edits and worktrees. The release includes application changes already on `main`.
2. Use Python 3.11+. Set the version in `Cargo.toml`, update `Cargo.lock`, and run
   `python scripts/build.py --prepare -s "Release summary"` to write `version.json`.
   Commit only those three release metadata files on top of the fetched main commit.
3. Push that commit directly with `git push origin HEAD:main`, using a normal
   fast-forward push. If main advances, integrate it and review the combined changes
   before retrying. If repository rules reject direct pushes, report the rejection;
   a PR is not an automatic fallback.
4. Wait for successful Windows push CI on the exact current `origin/main` SHA.
   From a clean checkout of that commit, run `python scripts/build.py --dry-run`,
   then `python scripts/build.py` to fetch the CI-built assets, publish them, and
   notify Discord.
5. Verify the tag points to the release commit and all three assets are uploaded.
   Inspect `python scripts/notify_discord.py --dry-run` before a manual announcement.
   Confirm Discord returned a message ID and that the message attaches
   `veluno-setup.exe` or links to that installer on the matching release. Retain
   the release URL and message ID in the result.

Publication does not rebuild. It downloads the `Veluno-Windows` artifact from the
passing push CI run for the release SHA, verifies it against that run's `SHA256SUMS`,
and pins the tag to that SHA. CI artifacts expire after 90 days; a release older than
that needs a CI rerun first. `gh` is required even with `--skip-github`. GitHub failures stop the Discord announcement; `--skip-github` is the
explicit Discord-only path. Preparation and publication are separate commands;
`--dry-run` validates without building or publishing.

## Webhook Configuration

- The webhook URL is resolved in order from:
  1. `DISCORD_WEBHOOK_URL` environment variable
  2. `scripts/discord_webhook.txt` or `discord_webhook.txt` (excluded in `.gitignore`)
  3. `%LOCALAPPDATA%\Veluno\discord_webhook.txt` (shared across all git worktrees)
- **Security Rule**: Never commit `discord_webhook.txt`.
- Channel: `#veluno-releases` (`1561759599609258109`) in the Felary Discord server.
