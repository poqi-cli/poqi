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

The locked Linux application graph uses the asynchronous Secret Service provider over zbus `async-io` and contains no `dbus-secret-service`, `dbus`, or `libdbus-sys` package. The previous bundled D-Bus appendix and its `libdbus-sys` source guards were removed only after that graph and the cargo-about report were checked. Linux notice generation first rejects any of those packages in the normal application graph, then cargo-about applies the license allowlist. Reintroducing one of them requires a fresh license and bundled-source review before release. The Linux compatibility test temporarily builds the previous provider in an isolated scratch copy; that test-only graph is not part of the application or release notice graph.
