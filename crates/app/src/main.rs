#![warn(clippy::all, clippy::pedantic)]

mod bootstrap;
mod cli;
mod diagnostics;
mod runtime;
mod settings;

use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    runtime::run().await
}

#[cfg(test)]
mod tests;
