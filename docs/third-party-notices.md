# Third-party release notices

Every release archive contains `THIRD-PARTY-LICENSES.txt`. The release workflow generates its Rust dependency section with cargo-about 0.9.1 and then appends notices for native source that Cargo metadata cannot describe.

`about.toml` is the fail-closed license allowlist. `about.hbs` is the plain-text report template. A new external license must be reviewed before it is accepted; the narrow `option-ext` and `ring` entries are the current exceptions. poqi workspace crates set `publish = false` so cargo-about's private-package handling omits the application's own custom license from this third-party report.

`third-party/native-dependencies.json` pins the reviewed crates whose bundled native source needs an appendix and hashes the exact license-bearing sources. `third-party/native-licenses.txt` contains the corresponding upstream notices and records their source locations. If a guarded crate version or source changes, generation stops until its bundled source and notices have been reviewed and both files are updated.

Generate a report from the repository root with a pinned cargo-about installation:

```powershell
cargo install --locked --version 0.9.1 --features cli cargo-about
./scripts/generate-third-party-notices.ps1 `
  -TargetTriple x86_64-pc-windows-msvc `
  -OutputPath target/release/THIRD-PARTY-LICENSES.txt
```

The supported triples are `x86_64-unknown-linux-gnu`, `aarch64-apple-darwin`, `x86_64-apple-darwin`, and `x86_64-pc-windows-msvc`. Generation uses the locked application dependency graph and fails on unresolved or unaccepted licenses. The result records upstream terms; it is not a legal certification.

The Linux dependency chain currently statically links D-Bus 1.14.4 through `libdbus-sys` and selects AFL-2.1 from its dual-license terms. [AFL-2.1 section 9](https://spdx.org/licenses/AFL-2.1.html) requires reasonable efforts to obtain express assent to its terms, which distributing this notice file alone does not establish. Release publication must remain paused until the owner approves a compliant assent mechanism or a separately reviewed dependency change removes this path. An alternative Linux credential-store dependency is under investigation; this notice change does not implement it.
