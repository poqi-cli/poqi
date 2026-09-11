use thiserror::Error;

#[derive(Debug, Error)]
pub enum TestDataError {
    #[error("Docker is not available. Please install Docker Desktop and ensure it's running.\nVisit https://docs.docker.com/get-docker/ for installation instructions.")]
    DockerNotAvailable,

    #[error("Failed to start PostgreSQL container: {0}")]
    ContainerStartFailed(String),

    #[error("Failed to connect to database: {0}")]
    ConnectionFailed(String),

    #[error("Test data generation failed: {0}")]
    GenerationFailed(String),

    #[error("Operation cancelled by user")]
    Cancelled,

    #[error("Database error: {0}")]
    DatabaseError(#[from] poqi_db::DatabaseError),
}
