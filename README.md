# poqi

**PostgreSQL Query Interface** — a PostgreSQL terminal interface built with Rust.

Browse schemas, run SQL, edit rows, and explore data with optional local semantic search. Navigate with WASD, arrows, or the mouse.

## Build from source

Install poqi from a source checkout with a Rust toolchain:

```sh
cargo install --path crates/app --locked
poqi
```

See [current status](docs/current-status.md) for supported behavior and [architecture](docs/architecture.md) for the workspace structure.

## Connect to PostgreSQL

Run `poqi` in a terminal, create a connection, and paste a PostgreSQL URL or enter host, port, user, password and database separately. Saved profiles can be opened again with Enter and edited with E. Failed connections keep the form open with an explanation; a profile is saved only after connection setup succeeds. Successful connections are saved with encrypted credentials.

```sh
poqi
```

The profile menu always opens first. Semantic search is off by default; enable it in Settings when you want to download and use the model. Headless login checks are optional: `poqi --check-connection --profile local`. See the [connection guide](docs/connections.md) for TLS, password handling and troubleshooting.

## License and responsibility

poqi uses the project-specific [poqi No-Sale Source License 1.0](LICENSE). It permits personal and internal business use, modification and free redistribution. Selling the software or modified versions, charging for copies, licenses or hosted access, and including them in paid offerings are prohibited. Optional support, training and work performed using the tool may be paid, provided the payment does not buy access to the software itself.

Distributed modifications must use the same terms and include corresponding source code or a free source download; publicly distributed versions must make that source publicly available. Private internal changes need not be published. Because the license restricts sales, poqi is described as source available, not OSI-approved open source.

This distribution is offered under the new license, subject to existing rights: the code present at commit `6f7dfa0fb051c94e661e325a5a97483980ac5780` remains available under MIT, including its permission to sell copies. A new repository, name or Git history does not revoke those rights; see the historical notice in [LICENSE](LICENSE). New contributions after that baseline use the new terms unless separately licensed. Dependencies, downloaded models and runtimes retain their own upstream licenses.

The software is provided "as is", without warranty, with liability excluded to the extent permitted by applicable law. Review SQL, choose appropriate database permissions and maintain backups before making changes to your data. No support, maintenance or acceptance of contributions is promised.

Contributions are welcome through pull requests; see [contributing](CONTRIBUTING.md).

## Release installers

The Linux x86_64 binary targets Ubuntu 24.04 (glibc 2.39); older distributions are
not validated. Other Linux systems can build from source. Saving connections
requires an available, unlocked Secret Service on Linux, Windows Credential
Manager on Windows, or Keychain on macOS. Native packages and credential storage
must pass the release checks on both supported macOS architectures.

Release packages and installers are published through [GitHub Releases](https://github.com/poqi-cli/poqi/releases) after their release checks pass. If no release is listed, build from source.

Windows: starting with v1.0.1, download `poqi-vX.Y.Z-windows-x86_64-setup.exe` from the release assets and open it. The installer needs no administrator rights, installs in `%USERPROFILE%\.poqi\bin`, adds `poqi` to your user PATH, and creates a Start menu shortcut that opens the app in a terminal. Reopen existing terminals after installation. Run a newer installer to update; uninstall through Windows Settings. Uninstall keeps your saved connections and application data. Portable ZIP archives remain available.

The v1.0.2 release includes the native setup, portable ZIP archive, and script installer. The setup is unsigned, so Windows may show an unknown-publisher warning.

For the first native setup, move any old ZIP/script program files out of the destination folder first. Setup refuses to overwrite an unregistered installation or unrelated files with the same names. Saved connection data does not need to be moved. Later native setup versions update the registered installation in place.

Windows PowerShell alternative:

```powershell
powershell -ExecutionPolicy Bypass -c "irm https://github.com/poqi-cli/poqi/releases/latest/download/install.ps1 | iex"
```

macOS or Linux:

```bash
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/poqi-cli/poqi/releases/latest/download/install.sh | sh
```

Then start the app:

```bash
poqi
```

Set up your database connection inside the app.

If `poqi` is not found after installation, restart your terminal and try again.

Saved connection URIs are encrypted in SQLite; the encryption key is kept in your operating system credential store. Semantic search downloads model/runtime files only after you enable it in Settings. See [connection and TLS behavior](docs/current-status.md#what-works-today) before choosing connection options.

## Troubleshooting

Check your installation:

```bash
poqi --doctor
```

`--doctor` checks local configuration. Use `poqi --check-connection --profile local` to test an actual login without opening the TUI. Run `poqi --help` for connection examples.

## Website development

The configured product-site address is [poqi — PostgreSQL terminal client](https://poqi-cli.github.io/poqi/), with an [installation and connection guide](https://poqi-cli.github.io/poqi/getting-started/). The source lives in [website/](website/README.md), with a separate Astro build and GitHub Pages workflow.
