# poqi Build Guide

This file is a navigation hub. Every implementation or roadmap detail now lives in a focused doc—use these entry points instead of re-reading the entire tree.

---

## Where to Look
- **What exists today** → `docs/current-status.md` (capabilities, quick start, implementation notes, known gaps).
- **High-level architecture** → `docs/architecture.md` (crate layout, runtime flow, constraints).
- **Roadmap & design bets** → `docs/future-vision.md` (autocomplete, semantic search, CRUD flows, packaging).
- **Test matrix** → `docs/testing-guide.md` (workspace commands, optional Docker suites, targeted smoke tests).
- **Contributor standards** → `AGENTS.md` (doc-first workflow, lint/test gates, WASD model guardrails).

Keep these documents in sync—if you implement or remove behaviour, update the matching reference in the same pull request.

---

## Quick Links
- **Run the app**: follow `current-status.md#quick-start`.
- **Semantic model assets**: the Granite model/tokenizer download plus the automatic ONNX Runtime cache (`<config>/runtimes/onxxruntime/1.23.0/<platform>`) and `ORT_DYLIB_PATH` override notes live under `current-status.md#quick-start`.
- **Configure profiles/keymaps**: see `current-status.md#store--configuration`.
- **Understand upcoming work**: scan `future-vision.md#roadmap-themes`.
- **Testing checklist**: `testing-guide.md#workspace-suite`.
- **Quality gates**: `AGENTS.md` → `cargo fmt`, `cargo clippy --workspace --locked -- -D warnings`, `cargo test --workspace`.

---

## Contributing Flow
1. Read `current-status.md` to understand the truth today.
2. Consult `future-vision.md` before expanding scope; update it if the roadmap shifts.
3. Keep `architecture.md` accurate whenever you touch crate boundaries, messaging, or runtime flow.
4. Update `testing-guide.md` when you add, remove, or rename checks.
5. Run the required commands (fmt, clippy, test) and mention them in your testing log.

This split keeps each document short and keeps false claims out of the build guide.
