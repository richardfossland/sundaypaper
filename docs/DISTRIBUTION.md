# SundayPaper — Distribution

How SundayPaper is built, signed, and shipped. Mirrors the SundayEdit /
SundayStage flow.

## Pipeline

`.github/workflows/release.yml` runs on a `v*` tag (or manual dispatch) and
builds macOS (Apple Silicon) + Windows via `tauri-apps/tauri-action`, producing
a **DRAFT** GitHub Release with the installers and the updater `latest.json`.

Cut a release:

```sh
npm version <patch|minor|major>          # bumps package.json
# bump src-tauri/tauri.conf.json "version" to match (and Cargo.toml if desired)
git commit -am "chore: release vX.Y.Z"
git tag vX.Y.Z && git push origin vX.Y.Z
```

Review the draft release, then publish it. Installed apps then pick up the
update from the `releases/latest/download/latest.json` endpoint configured in
`tauri.conf.json`.

## Secrets

| Secret                               | Required? | Purpose                                                                                                                 |
| ------------------------------------ | --------- | ----------------------------------------------------------------------------------------------------------------------- |
| `TAURI_SIGNING_PRIVATE_KEY`          | **Yes**   | Signs the updater bundle so existing installs trust the update. Value = contents of `~/.tauri/sundaypaper_updater.key`. |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | **Yes**   | Password for that key — **empty** for this key.                                                                         |
| `APPLE_CERTIFICATE`                  | No        | base64 `.p12` Developer ID Application cert.                                                                            |
| `APPLE_CERTIFICATE_PASSWORD`         | No        | Password for the `.p12`.                                                                                                |
| `APPLE_SIGNING_IDENTITY`             | No        | e.g. `Developer ID Application: Richard Fossland (TEAMID)`.                                                             |
| `APPLE_ID`                           | No        | Apple ID email for notarization.                                                                                        |
| `APPLE_PASSWORD`                     | No        | App-specific password for notarization.                                                                                 |
| `APPLE_TEAM_ID`                      | No        | Apple Developer Team ID.                                                                                                |

Without the `APPLE_*` secrets the macOS build is **unsigned** — fine for
internal testing (first launch needs right-click → Open to clear Gatekeeper);
add them later for a public, notarized release. Windows is unsigned until a
Windows code-signing cert is added (follow-up).

## Updater keys

- Private key: **`~/.tauri/sundaypaper_updater.key`** (empty password) — kept
  **outside** the repo. Only the public key is committed (in
  `tauri.conf.json` → `plugins.updater.pubkey`).
- The matching `TAURI_SIGNING_PRIVATE_KEY` repo secret must hold the private
  key's contents for CI to produce verifiable updater artifacts.
- Lose the private key and existing installs can no longer auto-update — keep a
  backup.

## Deliberately deferred

- **Apple notarization + Windows code signing** (needs paid certs/secrets).
- **Universal / Intel macOS build** (currently arm64 only).
- **Bundling pdfium**: the `pdf` cargo feature (PDF render/extract) is **off**
  in release builds, so PDF tools report "feature not enabled". Shipping the
  pdfium dynamic library as a bundled resource + binding to it at runtime is a
  follow-up; lopdf-based split/merge/rotate/extract and pdfium text/render are
  already implemented behind the feature.
- **Auto-update end-to-end test** (needs two published releases).
