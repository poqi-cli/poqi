# Testing Guide

All commands run from the repo root. Keep this file up to date whenever you add or remove checks.

---

## Workspace Suite (default)
Run the full test matrix and keep it green before opening a PR:
```bash
cargo test --workspace
```
What it covers today:
- Config/CLI regression tests (startup import ordering, repeated loads, complete DB/keymap persistence, saved runtime preferences, and CLI overrides).
- Store/catalog/db/search crates (stub guards, connection handling, store schema initialization and unsupported-schema rejection).
- Store regressions for plaintext and malformed encrypted records, authentication failures, missing-key failure with populated stores, and key recreation only for empty stores.
- UI headless tests (navigation, Unicode, stale result fencing, rapid table selection, worker errors, cancellation, alias editing, and content-based embedding cache reuse).
- Semantic adapter tests (tokenizer special tokens, tensor shape, CLS extraction, dimensions, and normalization). Real ONNX latency and retrieval quality require separate model runs.
- Doctests for configuration defaults.

PR/`dev`/`main` CI runs Ubuntu fmt, locked workspace Clippy/tests (including PostgreSQL), `cargo audit`, a debug CLI build/help check, installer dry runs, and immutable-workflow-reference checks. A separate native credential-store matrix runs store/config regressions and a native save/reopen smoke test on Linux, Windows, macOS ARM64, and macOS Intel. Releases call both workflows for the exact source SHA and require them, plus every extracted-package smoke check, before publication. Merely adding the jobs does not establish that a particular commit passed them; retain the hosted run evidence before release.

`scripts/test-release-package.ps1` checks package contents, help and tag/version agreement. Unix packages also run isolated `--doctor`; the Linux job tests the extracted binary against disposable PostgreSQL. Windows package checks do not run `--doctor`, because the profile store uses Known Folders and cannot be redirected by the config-directory override. Native store behavior is covered separately by the credential-store workflow.

> Need logs? Append `-- --nocapture`.

### Docker-backed integration tests
Postgres-powered specs (catalog/db/engine/testdata) are skipped unless you opt in:
```bash
POQI_ENABLE_TESTCONTAINERS=1 cargo test --workspace
```
Set the flag to `1/true/yes/on` (case-insensitive) when Docker is available. CI enables it on Linux runners only.
These tests cover physical row identity across partitions/inheritance, source-column aliases, unchanged aggregate/DISTINCT/order semantics, views, OID formatting, CTID reuse after VACUUM FULL, composite primary keys, stale-row rollback, dotted/quoted custom types, membership-sensitive read-only projections, server cancellation, connection loss without replay, and demo-container readiness/cleanup.
They also cover SQL NULL versus literal text, quoted relation CRUD, materialized/foreign relation discovery and restricted-role catalog visibility. Headless tests cover whole-token completion, cursor refresh, aliases, strict semantic thresholds and bounded cancellation delivery.

Before merging `dev` into `main` in `poqi-cli/poqi`, verify the new stable application version in `crates/app/Cargo.toml`, the matching `Cargo.lock`, and the user-facing pull-request summaries. Only a merged same-repository `dev`-to-`main` pull request can trigger the exact-commit quality, native-store, and package gates. Tag and GitHub Release creation follow only when those hosted gates pass.

Before announcing the first release, retain hosted evidence for all four platform archives and their checksum/version agreement. Exercise the published installer URLs and resulting `PATH` behavior, then verify that each installed application can save a connection, exit, and reopen it through the platform's native credential store. Source builds and local package fixtures do not replace these installed-package checks.

---

## Targeted Runs
- **Single crate** (faster during iteration): `cargo test -p poqi-config`
- **Specific UI test**: `cargo test -p poqi-ui --lib event_loop_exits_on_quit_binding`
- **Single test with logs**: `cargo test -p poqi-ui --lib space_hold_skips_space_after_nav_usage -- --nocapture`

Use these when you are iterating on a focused area and want quicker feedback than the full workspace run.

For a manual model-download diagnosis, set `POQI_MODEL_DOWNLOAD_PROBE_DIR` to an isolated writable directory and run `cargo test -p poqi-ui pinned_model_download_probe --locked -- --ignored --nocapture`. This uses the production download and integrity checks against the pinned FP32 model (about 390 MB). It is intentionally excluded from the normal offline test suite; it does not change the application model cache or prove inference works.

---

## Manual Smoke (binary)
Automated coverage is still light for bootstrap flows. Before publishing a release or when touching bootstrap/config code, run:
```bash
cargo run -- --show-config
```
This compiles the binary, loads config + CLI overrides, and prints the resolved settings so you can confirm paths and keymap profile before starting the TUI.
Diagnostics initialize the normal SQLite profile store. `POQI_CONFIG_DIR` redirects configuration files but does not isolate that store. Unit tests use explicit temporary store paths.

For terminal lifecycle changes, also exercise the native profile menu and TUI, including failed connection recovery and exit during semantic inference. Headless tests do not establish terminal restoration on every supported OS.

---

## Quality Gates Reminder
Per `AGENTS.md`, every PR must record the following commands (additional focused tests are welcome but not a substitute):
```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace
```
