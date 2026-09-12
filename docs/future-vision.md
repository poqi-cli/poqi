# poqi — Future Vision

Purpose: capture the aspirational roadmap so contributors know which big rocks are ahead and how they connect. Everything described here is **not** finished yet—check `docs/current-status.md` for the shipping feature set.

---

## Mission & Pillars
- **Mission**: build a cross-platform PostgreSQL CLI/TUI for developers who want WASD-first navigation, fast query loops, and on-device discovery tools (autocomplete, semantic search, CRUD helpers).
- **Speed-first UX**: async I/O, prepared statements, streaming COPY, and progressive rendering keep round-trips short.
- **Gamer-grade input**: WASD + arrows, Shift fast-scroll, `F` as the context action, full keyboard rebinds.
- **Intelligent assistance**: AST-aware autocomplete plus fuzzy + semantic search to surface the right table, column, or snippet instantly.
- **Safe experimentation**: Guided CRUD panels with previews, statement timeouts, and easy cancellation so developers can explore without fear.
- **Portable delivery**: signed binaries and package feeds for macOS, Windows, and Linux.

---

## Strategy at a Glance
| Theme | Goal | Key Dependencies |
| --- | --- | --- |
| Workspace foundations | Keep crates layered, inputs testable, and telemetry optional. | `tokio`, `deadpool-postgres`, `rusqlite`, `tracing` |
| Query & CRUD experience | Rich editor, cancellation, safe INSERT/UPDATE/DELETE flows. | pg_query, CRUD planner, Result grid upgrades |
| Search & autocomplete | Merge AST context, fuzzy index (tantivy), and semantic priors. | `pg_query`, `tantivy`, Granite ONNX, ANN index |
| Semantic discovery | Ctrl/⌘-K palette, related tables, snippet surfacing. | Granite via `ort`, USearch/HNSW |
| Packaging & polish | Themes, accessibility, installers, first-run wizard. | Ratatui theming, cargo-dist, platform packaging |

---

## Roadmap Themes

### 1. Foundations & Store
- Expand `poqi_store` beyond profiles to favorites, recents, snippets, search metadata, and keymap packs (SQLite WAL mode, versioned migrations).
- Background catalog crawl + incremental refresh to keep metadata warm between sessions.
- Offline cache for search/ANN indexes with rebuild triggers on schema drift.

### 2. Query, Editor, and CRUD
- Multi-tab editor with history, selection execution, and query cancellation (`Ctrl+C`).
- Guided CRUD flows: insert/update/delete forms derived from schema metadata, WHERE builders with dry-run previews, confirmation UX for destructive actions.
- Push inline grid editing beyond single-table results: teach the planner to refresh joined/aggregated queries safely, surface INSERT, and add guard rails for multi-row edits.
- Result grid upgrades: column resize, copy/export wiring to existing COPY backend, selectable cell ranges.

### 3. Autocomplete & Fuzzy Search
- Extend the shipped shallow FROM/JOIN alias scope to nested queries, CTE outputs, WINDOW and RETURNING using pg_query context.
- Snippet engine with tab stops (SELECT template, JOIN FK template, INSERT/UPDATE/DELETE scaffolds).
- Hybrid ranking formula combining fuzzy prefix, graph proximity (FK neighbors), usage recency, and optional semantic priors.
- Tantivy-backed command palette for keyword search when semantic index is unavailable.

### 4. Semantic Search & Ranking
- Extend the on-device Granite Embedding 97M Multilingual R2 integration beyond RAM re-ranking of the focused table's `SELECT *` preview.
- Build/persist ANN indexes (USearch or hnsw_rs) that map tables, columns, snippets, and docstrings to vectors; expose rebuild + health commands.
- Ctrl/⌘-K palette merging semantic, fuzzy, and graph scores; returning results with actionable shortcuts (focus schema, open table, insert snippet).
- Related-tables sidebar that clusters FK + semantic neighbors for quick exploration.

### 5. Packaging & First-Run Experience
- Theme packs (dark/light/high contrast) with data-driven palettes and reduced-motion option.
- Accessibility polish: keyboard parity, focus outlines, wide glyph handling, localization-friendly prompts.
- Windows v1.0.1 uses a per-user NSIS EXE installer alongside the portable ZIP, without requiring a package manager or administrator rights. Signing and package feeds remain future work: Homebrew tap + notarized pkg (macOS), winget/MSI (Windows), deb/rpm (Linux).
- Publish an npm CLI wrapper that installs the verified platform release archive and links to the website download options.
- Extend first-run onboarding to theme selection, telemetry opt-in, and semantic model download/license acceptance.

---

## Key Technical Bets

### Autocomplete Evolution
- pg_query AST + incremental parser to detect context at each keystroke.
- Metadata enrichment: column types/nullability, FK relationships for JOIN ON suggestions, usage metrics for ranking.
- Latency target ≤10 ms: debounce input, pre-scan buffer, cache alias scopes, cap candidate counts, and precompute predicate snippets (≤8 per table).

### Semantic Stack
- The current panel uses Granite Embedding 97M Multilingual R2 with plain query text, column-labelled row documents, its tokenizer, and normalized 384-dimensional CLS embeddings. CPU uses compact INT8, Windows DirectML uses FP32, and Auto selects the available hardware backend. Asset revisions are immutable, downloads require checksum verification, and row-content vectors are cached in RAM across searches.
- Next steps: evaluate model-supported dimensionality reduction and persistent ANN APIs (build/load/save, incremental inserts, cosine search) via `usearch` or `hnsw_rs`.
- Calibrate relevance thresholds on representative Finnish/English table queries for both CPU INT8 and GPU FP32, including unrelated queries. Evaluate field weighting before changing models; exact numeric predicates belong in SQL.

### Storage Layout (future)
Suggested SQLite tables (subject to refinement): `profiles`, `favorites`, `recents`, `snippets`, `settings`, `search_meta` (engine path, dim, updated_at), `tantivy_meta`, `keymaps`, `macros`. All migrations must be idempotent and versioned under `poqi_store`.

### Security & Resilience
- Redact literals in logs by default; allow opt-in verbose logging with warnings.
- Recover gracefully from connection drops by rebuilding the pool for subsequent requests. Never replay a dispatched write whose commit result is unknown; retry only work proven safe. Persist search indexes atomically.

### Accessibility & Internationalization
- Keyboard parity (arrow equivalents for WASD, configurable leader keys for non-US layouts).
- High-contrast theme plus reduced-motion flag that disables blink/animation.
- UTF-8 safe layout calculations with ellipses for wide glyph truncation.

---

## Milestones (Reference Only)
| Milestone | Focus | Exit Criteria |
| --- | --- | --- |
| **M1 Foundations** | Bootstrap crates, config loader, profile selector, schema crawl v1, Select Top action. | Connect → browse → F runs SELECT TOP reliably. |
| **M2 Execute & Render** | Rich editor, cancellation, improved results grid, performance baselines. | Smooth query loop with WASD + Shift fast-scroll. |
| **M3 CRUD** | Insert/Update/Delete forms, guard rails, previews. | Fast, safe CRUD with consistent `F`/`R` actions. |
| **M4 Autocomplete** | pg_query context, snippets, ranking hooks, perf targets. | IDE-like completions that feel instant. |
| **M5 Semantic Search** | Extend the ONNX embedder and RAM re-ranking with an ANN index, Ctrl/⌘-K palette, and hybrid ranker. | “Go to anything” palette with semantic + fuzzy relevance once ANN + palette ship. |
| **M6 Polish & Packages** | Themes, accessibility, installers, first-run wizard, keymap editor. | Ready-to-ship binaries with delightful onboarding. |

Use this table to align planning conversations; adjust milestone contents in this file whenever scope shifts.

---

## Working With This Doc
- When you start implementing a roadmap item, link the PR back to the relevant section and trim/adjust language once reality changes.
- If you introduce a new strategic bet (e.g., alternative semantic model, new packaging flow), document the intent and constraints here before writing code.
- Keep the text terse—this file should remain an approachable overview, not a dumping ground of every idea.
