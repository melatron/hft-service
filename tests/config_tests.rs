use hft_service::config::Config;

#[test]
fn test_default_config_loading() {
    let config = Config::new().expect("Should load default config");

    // Verify default values from Config.toml exist
    assert!(!config.server.host.is_empty());
    assert!(config.server.port > 0);
    assert!(!config.log.level.is_empty());
}

#[test]
fn test_config_serialization() {
    let config = Config::new().expect("Should load config");

    // Test that config can be serialized (useful for debugging/logging)
    let serialized = serde_json::to_string(&config).expect("Should serialize config");
    assert!(serialized.contains("server"));
    assert!(serialized.contains("log"));
    assert!(serialized.contains("host"));
    assert!(serialized.contains("port"));
    assert!(serialized.contains("level"));
}

#[test]
fn test_config_debug_output() {
    let config = Config::new().expect("Should load config");

    // Verify that config can be debug printed (useful for troubleshooting)
    let debug_output = format!("{:?}", config);
    assert!(debug_output.contains("ServerConfig"));
    assert!(debug_output.contains("LogConfig"));
    assert!(debug_output.contains("host"));
    assert!(debug_output.contains("port"));
    assert!(debug_output.contains("level"));
}

#[test]
fn test_config_structure() {
    let config = Config::new().expect("Should load config");

    // Verify the expected structure exists
    assert!(config.server.port > 0, "Port should be valid");

    // Verify log level is one of the expected values
    let valid_levels = ["trace", "debug", "info", "warn", "error"];
    assert!(
        valid_levels.contains(&config.log.level.as_str()),
        "Log level should be valid: {}",
        config.log.level
    );
}
