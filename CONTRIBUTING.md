# Contributing to poqi

By submitting a new contribution for inclusion in poqi, you agree to license your contribution under the [poqi No-Sale Source License 1.0](LICENSE). Only submit work that you have the right to contribute under those terms, and preserve notices for any previously licensed material. You retain copyright in your contributions. These terms do not retroactively change earlier contributions or third-party licenses. Pull requests are reviewed at the maintainers' discretion; submission does not guarantee acceptance, support or maintenance.

Read [current status](docs/current-status.md) before proposing behavior changes. Architecture belongs in [architecture](docs/architecture.md), and planned features belong in [future vision](docs/future-vision.md).

Keep changes focused, preserve the documented keyboard navigation, and update the relevant documentation when behavior changes. Use clear pull-request titles and descriptions: GitHub generates release notes from merged pull requests between releases. `CHANGELOG.md` links to that history; do not add development entries there. Add regression coverage for bug fixes. Rust workspace crates use the `poqi-*` prefix.

Run the required checks before opening a pull request:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace
```

Docker-backed PostgreSQL tests are opt-in locally: set `POQI_ENABLE_TESTCONTAINERS=1` and `POQI_TESTCONTAINERS_PG_TAG=18-alpine`. CI runs these tests on Ubuntu for `dev`, `main` and pull requests.

Develop on `dev`. A release is initiated only by merging a same-repository pull request from `dev` into `main` in `poqi-cli/poqi`. Before that merge, set a new stable `major.minor.patch` version in `crates/app/Cargo.toml` and update `Cargo.lock`. The workflow validates the merged commit, builds all four platform packages, and publishes `v<version>` only after its gates pass. Direct pushes, other source branches, forks, and unmerged pull requests do not publish releases.

Use synthetic data in tests, screenshots, logs and issue reports. Remove credentials and personal data before sharing a reproduction.
