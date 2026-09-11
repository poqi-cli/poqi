#![warn(clippy::all, clippy::pedantic)]

#[derive(Debug, Clone)]
pub struct RankedCandidate {
    pub id: String,
    pub score: f32,
}

#[derive(Debug, Default)]
pub struct HybridRanker;

impl HybridRanker {
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    #[must_use]
    pub fn rank(&self, _query: &str) -> Vec<RankedCandidate> {
        Vec::new()
    }
}

#[cfg(test)]
mod tests;
