#![warn(clippy::all, clippy::pedantic)]

use anyhow::Result;

#[derive(Debug, Clone)]
pub struct FuzzyCandidate {
    pub id: String,
    pub score: f32,
}

#[derive(Debug, Default)]
pub struct FuzzyIndex;

impl FuzzyIndex {
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Execute a fuzzy search query against the indexed catalog.
    ///
    /// # Errors
    /// Returns an error when fuzzy search integration is implemented and encounters a failure.
    pub fn search(&self, _query: &str, _limit: usize) -> Result<Vec<FuzzyCandidate>> {
        Ok(Vec::new())
    }
}

#[cfg(test)]
mod tests;
