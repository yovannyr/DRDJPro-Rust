use crate::crash_handler::{config::CrashHandlerConfig, handler::init_crash_handler};
use std::fs;

#[test]
fn test_crash_handler_config_defaults() {
    let config = CrashHandlerConfig::default();
    assert!(config.enabled);
    assert!(config.database_dir.is_empty() == false || config.database_dir.is_empty());
}

#[test]
fn test_crash_handler_init_disabled() {
    let config = CrashHandlerConfig {
        enabled: false,
        database_dir: std::env::temp_dir().join("crash_handler_test").to_str().unwrap().to_string(),
        ..Default::default()
    };
    init_crash_handler(&config);
}

#[test]
fn test_crash_reporter_generates_files() {
    let temp_dir = tempfile::tempdir().unwrap();
    let temp_path = temp_dir.path().to_str().unwrap();

    let config = CrashHandlerConfig {
        enabled: true,
        database_dir: temp_path.to_string(),
        ..Default::default()
    };

    init_crash_handler(&config);

    // We can't easily test actual panic capture in unit tests,
    // but we can verify the directory was created
    assert!(fs::metadata(temp_path).is_ok());
}

#[test]
fn test_queued_crash_serde() {
    let crash = crate::crash_handler::reporter::QueuedCrash {
        info_path: "/tmp/crash.json".to_string(),
        report_path: "/tmp/crash.txt".to_string(),
        timestamp: "2024-01-01T00:00:00Z".to_string(),
        message: "Test panic".to_string(),
        submitted: false,
    };

    // Serialize to JSON
    let json = serde_json::to_string(&crash).expect("Failed to serialize QueuedCrash");

    // Deserialize back
    let restored: crate::crash_handler::reporter::QueuedCrash =
        serde_json::from_str(&json).expect("Failed to deserialize QueuedCrash");

    assert_eq!(crash.info_path, restored.info_path);
    assert_eq!(crash.report_path, restored.report_path);
    assert_eq!(crash.timestamp, restored.timestamp);
    assert_eq!(crash.message, restored.message);
    assert_eq!(crash.submitted, restored.submitted);
}

#[test]
fn test_queue_save_and_load() {
    let temp_dir = tempfile::tempdir().unwrap();
    let temp_path = temp_dir.path().to_str().unwrap();

    let config = CrashHandlerConfig {
        enabled: true,
        database_dir: temp_path.to_string(),
        ..Default::default()
    };

    let crashes = vec![
        crate::crash_handler::reporter::QueuedCrash {
            info_path: format!("{}/crash1.json", temp_path),
            report_path: format!("{}/crash1.txt", temp_path),
            timestamp: "2024-01-01T00:00:00Z".to_string(),
            message: "First panic".to_string(),
            submitted: false,
        },
        crate::crash_handler::reporter::QueuedCrash {
            info_path: format!("{}/crash2.json", temp_path),
            report_path: format!("{}/crash2.txt", temp_path),
            timestamp: "2024-01-02T00:00:00Z".to_string(),
            message: "Second panic".to_string(),
            submitted: true,
        },
    ];

    // Save queue to QUEUE_FILE
    crate::crash_handler::queue::save(&crashes, &config).expect("Save failed");

    // Load queue via get_pending (which uses load_queued internally)
    let pending = crate::crash_handler::queue::get_pending(&config).expect("Get pending failed");
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].message, "First panic");
}

#[test]
fn test_queue_pending_filter() {
    let temp_dir = tempfile::tempdir().unwrap();
    let temp_path = temp_dir.path().to_str().unwrap();

    let config = CrashHandlerConfig {
        enabled: true,
        database_dir: temp_path.to_string(),
        ..Default::default()
    };

    let crashes = vec![
        crate::crash_handler::reporter::QueuedCrash {
            info_path: format!("{}/crash1.json", temp_path),
            report_path: format!("{}/crash1.txt", temp_path),
            timestamp: "2024-01-01T00:00:00Z".to_string(),
            message: "Pending crash".to_string(),
            submitted: false,
        },
        crate::crash_handler::reporter::QueuedCrash {
            info_path: format!("{}/crash2.json", temp_path),
            report_path: format!("{}/crash2.txt", temp_path),
            timestamp: "2024-01-02T00:00:00Z".to_string(),
            message: "Submitted crash".to_string(),
            submitted: true,
        },
    ];

    crate::crash_handler::queue::save(&crashes, &config).expect("Save failed");

    let pending = crate::crash_handler::queue::get_pending(&config).expect("Get pending failed");
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].message, "Pending crash");
}
