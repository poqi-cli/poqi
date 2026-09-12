# poqi Architecture

**Purpose**: Provide a concise map of the poqi workspace, core design choices, and how the runtime fits together. Detailed feature behaviour lives in `docs/current-status.md`; roadmap items stay in `docs/future-vision.md`.

---

## 1. Core Principles
- **Async IO, sync UI**: All database, file-system, and clipboard work happens on async tasks, while the Ratatui event loop stays synchronous for deterministic redraws.
- **Layered crates**: Presentation (`ui`), orchestration (`app`), business logic (`engine`), data access (`db`), metadata (`catalog`), config/store, and tooling each live in their own crates to keep dependencies one-directional.
- **Testable foundations**: `db`, `catalog`, `store`, `config`, and search crates can run headless unit tests; integration tests use `testcontainers` + PostgreSQL 18.
- **Developer-first defaults**: Profiles and config are easy to edit, credentials are entered manually; encrypted local persistence uses the native credential store.
- **Progressive enhancement**: OSC-8 links, mouse input, and clipboard helpers are enabled only when the terminal reports support.

---

## 2. Workspace & Layering
```
poqi/
├─ crates/
│  ├─ app/            # binary entry point + bootstrap
│  ├─ ui/             # Ratatui views, WASD input, engine worker wiring
│  ├─ engine/         # DatabaseEngine (query + CRUD adapters)
│  ├─ db/             # deadpool-postgres pool, COPY helpers
│  ├─ catalog/        # tables + columns snapshot (with FK placeholder types)
│  ├─ search/
│  │  ├─ completion/  # production SQL autocomplete
│  │  ├─ fuzzy/       # stub (returns [])
│  │  ├─ semantic/    # Granite ONNX bridge + ranking helpers
│  │  └─ ranker/      # stub (returns [])
│  ├─ store/          # rusqlite profile DB
│  ├─ config/         # TOML loader + keymap parsing
│  ├─ observability/  # tracing subscriber setup
│  └─ tools/          # testdata and test-support helpers
├─ docs/
└─ CHANGELOG.md
```

**Layer boundaries**
- Foundation: `config`, `store`, `db`, `observability`
- Shared logic: `engine`, `catalog`, `search/*`
- Presentation: `ui`
- Orchestration: `app`

All arrows point upward (no circular deps); binaries only exist in `crates/app`.

---

## 3. Runtime Flow

### Bootstrap
1. `app` loads config, applies the optional CLI keymap override.
2. `store` initializes `poqi.sqlite` and returns connection profiles (URIs decrypted only in memory).
3. Interactive startup always opens the profile menu. Only headless login checks resolve explicit CLI URL/profile, then environment, then saved primary. It supports saved-profile editing, URL or separate connection fields, and secure persistence. URI/TLS validation shares the database connector's preparation path. Pending profile edits are saved only after connection and catalog setup succeed.
4. A lightweight Ratatui loading overlay keeps the terminal occupied while the selected profile connects and the catalog hydrates, so the hand-off to the main UI never feels like a blank pause.
5. `db::Database` connects via deadpool, runs a health check, then `catalog` crawls schemas/tables/columns into a snapshot. App setup has an adjustable 15-second deadline and failures return to the prefilled connection form with credential-safe actionable hints. `--check-connection` performs only login/healthcheck and exits without a terminal session or semantic bootstrap.
6. `ui::App` launches with schemas, catalog snapshot, config, and a channel pair to an engine worker task; a process-scoped semantic bootstrap coordinator starts runtime/model verification only when enabled, after first draw or after saving an Off-to-enabled setting, keeps it alive across profile-menu transitions, and lets each UI session attach to current progress through the Semantic Search panel.

### Event Loop & Engine Worker
- **Main thread**: poll keyboard/mouse (`crossterm`), evaluate WASD layers, refresh completions, draw with Ratatui, and drain engine responses via `try_recv`.
- **Worker thread**: async task listens for `EngineCommand::{RunSql, SelectTop, Cancel}`, executes through `Engine` (usually `DatabaseEngine`), and replies with `EngineResponse::{Success, Error, Canceled}` tagged by `request_id`.

---

## 4. System Highlights

### UI & Input
- Panel layout: a left-hand schema browser (auto-fetches table previews), a top-right stack that splits the SQL editor (multi-line, completion popup) and the Semantic Search panel, and a bottom-right results grid (manual renderer with WASD navigation). The semantic pane now accepts natural-language prompts, fetches the focused table via `SelectTop`, and streams the rows to the semantic worker for ONNX ranking before the grid refreshes.
- Results grid drawing and the SQL editor now funnel through small helper modules (layout, scrollbars, hitboxes, wrapping, gutter, highlighting) so `app::App` stays lean while behaviour remains identical.
- Ratatui 0.30.2 powers the visuals (rounded borders, padding, scrollbars, underline colours). Always confirm the pinned Ratatui version before planning UI changes so new widget capabilities stay in play.
- Two interaction layers keep the WASD-first model predictable: `PanelSelect` (choose panel) and `PanelFocused` (send input to the active panel). Keymaps come from config (default profile mirrors WASD + arrows); mouse events are optional.
- The status bar participates in that selector and doubles as the Settings entry: pressing **Enter** while the status tile is highlighted (or clicking the status bar) opens a full-screen modal driven by `SettingsView` (it hydrates from the SQLite `settings` table, keeps pending copies/dirty markers, and swaps `UiLayer::Settings` in until the user presses Enter on "Save & Exit" (fixed at the bottom) or Esc / a click outside the modal to cancel). Left/Right (or A/D) nudge enums/numerics, Enter toggles editing for text fields, mouse wheel scrolls the list, mouse clicks select rows (choice fields cycle forward, editable fields begin editing), and saves apply theme, keymap, ranking options and `UiRuntimeSettings` immediately. Enabling semantic search from Off begins preparation on save. Changes between enabled runtime backends require a process restart.
- Semantic runtime/model preparation defaults to Off and remains process-scoped. User choices remain persisted. Preparation begins only after enabling in Settings, or after first draw on subsequent enabled launches. Auto prefers DirectML on Windows and CPU elsewhere. DirectML selects the FP32 artifact (about 415 MB including tokenizer); CPU selects INT8 (about 124 MB). Runtime selection precedes model download so only the selected model is fetched. Immutable revisions, SHA-256 verification, resumable downloads and progress remain in the bootstrap layer. A session worker owns a bounded cache of current row-content embeddings. Synchronous inference runs on a blocking worker with cancellation between batches; the terminal is restored before waiting for teardown.

### Engine & Database
- `DatabaseEngine` wraps a shared `db::Database`, enforces `statement_timeout` via `SET LOCAL`, and formats result rows into display-friendly strings. Connections require rustls TLS by default with native-root certificate-chain and hostname verification. Explicit `prefer` permits plaintext fallback but still verifies offered TLS; `disable` selects plaintext. URL `sslrootcert` uses custom-only trust and forces TLS. Client auth uses `sslcert`/`sslkey`. Operations are never replayed after an ambiguous connection failure.
- COPY helpers exist (`copy_out_csv`, `copy_out_csv_into`) but only the backend is wired today.
- SelectTop and targeted row refreshes use read-only transactions. Inline UpdateCell/DeleteRow require a selectable valid primary key, the target relation identity, physical table OID/CTID and row version (`xmin`), with an exactly-one-row check before commit. Primary-key values retain their PostgreSQL binary types and are bound as parameters; stale or changed identities fail. Source column names remain separate from aliases. Views, ordinary inheritance parents, tables without eligible keys and membership-sensitive or unsupported projections (including WHERE, LIMIT/OFFSET and row locks) remain read-only; SQL batches return an explicit error.

### Catalog & Metadata
- `poqi_catalog` discovers ordinary/partitioned tables, views, materialized views and foreign tables through permission-filtered PostgreSQL catalogs, retaining `information_schema.columns` semantics with catalog fallback. Its shared `QualifiedRelation` keeps schema and name separate through engine and UI operations.
- `CatalogSnapshot` seeds the completion service and schema browser without additional round-trips.
- The UI can issue a catalog refresh command that reruns the same crawl; successful editor-run DDL (CREATE/ALTER/RENAME/DROP on schemas/tables/views/foreign tables) marks the catalog stale and the next idle tick dispatches a refresh before updating the schema tree, table detail cache, and completion metadata.

### Search Modules
- `search/completion` provides keyword, clause, table, column, and predicate snippets powered by the catalog snapshot and light parsing+validation (`pg_query`).
- A shallow statement-local FROM/JOIN scope resolves simple aliases and rejects unknown/ambiguous qualifiers. Identifier quoting and whole-component replacement are shared with the editor contract; full nested scope resolution remains future work.
- Start keyword routing sits in `search/completion/src/start_keywords/*`, keeping SELECT/UPDATE/INSERT/DELETE/DDL state machines isolated while `lib.rs` handles shared helpers and validator wiring.
- `search/semantic` owns the Granite Embedding 97M Multilingual R2 ONNX loader, JSON tokenizer, normalized 384-dimensional CLS pooling, row text formatting and cosine scoring. CPU uses INT8; explicit DirectML uses FP32. `search/fuzzy` and `search/ranker` remain stubs.

### Store & Config
- `poqi_store` uses rusqlite (with bundled SQLite) under the OS local data directory (`%LOCALAPPDATA%` on Windows, `$XDG_DATA_HOME` or `~/.local/share` on Linux), storing profile names plus encrypted URIs, with a native-keyring key plus the `settings` table that tracks UI timing, semantic knobs, and theme/keymap preferences. Linux uses the asynchronous Secret Service provider over zbus `async-io`; each complete native call runs on a short-lived scoped OS thread outside the entered Tokio runtime, then the synchronous Store caller waits for its result.
- `poqi_config` loads TOML, merges defaults, resolves keymap profiles, and exposes `db.defaults` (timeout + page size) plus experimental `search` knobs.
- The application and all workspace crates use poqi names. Only current configuration and encrypted storage formats are supported; startup does not inspect or import previous application directories.

### Observability & Errors
- `poqi_observability` wires `tracing` with env-filter support and writes to `<config>/logs/poqi-semantic.log` so runtime/model bootstrap and semantic/DirectML diagnostics persist without polluting the TUI.
- Error handling favors user-facing strings through the `ui` status bar while logging detailed context for diagnosis.

---

## 5. Concurrency & Messaging
- One Tokio runtime drives the synchronous terminal event loop and async I/O workers; CPU inference is moved to blocking workers and canceled cooperatively between batches.
- `EngineCommand` / `EngineResponse` channels are unbounded `tokio::mpsc` pairs; each command carries a `request_id` so the UI can drop stale responses when a panel switches tasks.
- Metadata has separate display and SQL representations. Control and bidi characters are visibly escaped, with backslashes escaped for an unambiguous label. Raw identifiers still drive matching, quoting and completion insertion. Automatic previews pause for control/bidi relation names until a manual preview action.
- Results cache presentation widths when data changes. Each field's Unicode scan is limited to a 4 KiB UTF-8 prefix and display columns to 256 cells. Draws use this cache; edits and row hydration can grow widths until the next refresh rebuilds them. Display truncation never replaces raw result values.
- Model and runtime downloads share pre-write limits and resume validation. Models require their exact embedded size; runtime archives are capped at 512 MiB. HTTP deadlines are 30 seconds to connect, 60 seconds without read progress and two hours total. Invalid partials are removed; bounded partials from transient network failures can resume. A rejected range permits only one clean retry, and final SHA-256 verification remains mandatory.
- Cancellation is sent to PostgreSQL before retiring an active request. Cancellation tracks pending acquisition and the active client; a canceled client is discarded instead of returning to the pool. Request IDs reject stale responses, including old semantic results after newer SQL. Worker failures produce a response rather than leaving the UI waiting.
- A single five-second worker deadline covers cancellation registration, delivery and settling. Dispatched connection failures return immediately; pool recycling handles later requests without replaying the failed operation.
- A confirmed server cancellation becomes a normal canceled response only in the worker's requested-cancellation path. Before successful cancellation delivery, unsolicited cancellation and statement timeout remain errors. After delivery, SQLSTATE confirms an abort without relying on localized server text; a competing timeout has the same code and cannot be distinguished. Failed delivery and unknown outcomes remain errors; an already completed commit remains authoritative.

---

## 6. Constraints & Extension Points
- Single active database task: a new command requests cancellation of the previous request and observes its result before starting. Completed results remain authoritative; failure to confirm the outcome is reported explicitly. A connection failure after dispatch never replays the operation.
- No schema caching between launches; every session re-crawls the catalog to stay accurate.
- Connection-form saving is automatic after successful setup. AES-256-GCM protects entire URIs and the primary setting; a per-store random key lives in the native OS credential store. Nonces are fresh per write and authenticated context binds records to their store and identity. Missing keys fail closed while any saved connection remains; only an empty store can create a new key. Authentication failures never cause replacement keys or plaintext fallback. An initial TOML primary is removed from the active config only after encrypted persistence succeeds. Headless check URLs remain unsaved.
- Theme and keymap systems are data-driven today and already structured for future user packs.
- Search/fuzzy/semantic crates are intentionally isolated so new algorithms can be swapped in without touching the UI contract—see `docs/future-vision.md` for roadmap specifics.

---

## 7. References
- [`docs/current-status.md`](current-status.md) — definitive list of implemented behaviour, commands, and limitations.
- [`docs/future-vision.md`](future-vision.md) — planned capabilities (semantic search, CRUD forms, multi-tab editor, packaging).
- [`docs/testing-guide.md`](testing-guide.md) — how to run the optional Docker-backed suites.
