# Agent Instructions

These notes apply to every contribution in this repo.

## Documentation Ground Rules
- Treat `docs/current-status.md` as the source of truth for everything that already exists. If the behaviour changes, update it in the same PR.
- Use `docs/architecture.md` for high-level structure and `docs/future-vision.md` for roadmap items (search, autocomplete upgrades, semantic ranking, packaging, etc.).
- `docs/build-guide.md` simply points to the other docs; keep it in sync if filenames move.
- Keep the text lean: remove duplicate paragraphs instead of adding more words.

## Workflow Expectations
1. **Reference first**: Read the relevant doc section before touching code. Align plans with the documented scope.
2. **WASD input model**: Preserve the layered WASD/arrow navigation, context-aware actions, and editor keybindings unless the docs explicitly approve a change.
3. **Search & autocomplete**: Follow the semantic/fuzzy strategy in `docs/future-vision.md`; don’t add new tooling without updating the roadmap.
4. **Documentation sync**: Architecture or roadmap changes must update the matching doc in the same pull request.
5. **Scope awareness**: New crates or modules must match the workspace layout described in the docs.

## Code Quality Standards
- Use the Rust 2021 edition, forbid `unsafe_code`, and keep Clippy pedantic warnings at zero.
- Treat every compiler or Clippy warning as a hard error: resolve it immediately instead of suppressing it so the app stays warning-free.
- Large functions (>100 lines) should be split; don’t wrap `Result` around infallible code.
- Remove unused imports, dead code, and pointless `async fn`s.

### Required Commands
Run these before calling the work complete, and list them in your testing log:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace
```

If a more specific target applies (e.g., a single crate), you may run that in addition, but the workspace commands above remain the default gate.
If the local toolchain is missing (e.g., cmake/sentencepiece deps), note it clearly and the user will run the full suite locally.
