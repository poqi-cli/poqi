use super::*;

#[test]
fn init_tracing_can_be_called_multiple_times() {
    let temp_dir = tempfile::tempdir().expect("tempdir should be created");
    std::env::set_var("POQI_CONFIG_DIR", temp_dir.path());
    std::env::remove_var("RUST_LOG");
    init_tracing().expect("first init should succeed");
    // Second initialization should be ignored gracefully.
    init_tracing().expect("second init should also succeed");
    let log_path = temp_dir.path().join("logs").join("poqi-semantic.log");
    assert!(
        log_path.exists(),
        "log file should be created at {}",
        log_path.display()
    );
    std::env::remove_var("POQI_CONFIG_DIR");
}
