//! Integration tests for XTrade CLI commands

use std::process::Command;
use std::time::Duration;

/// Test that the demo command works correctly
#[tokio::test]
async fn test_demo_command() {
    let output = Command::new("cargo")
        .args(["run", "--", "demo"])
        .output()
        .expect("Failed to execute demo command");

    assert!(output.status.success(), "Demo command should succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Testing Binance WebSocket"),
        "Should show demo start message"
    );
    assert!(
        stdout.contains("Test Results Summary"),
        "Should show test results"
    );
    assert!(
        stdout.contains("Depth updates processed"),
        "Should show update count"
    );
}

/// Test that the help command works
#[test]
fn test_help_command() {
    let output = Command::new("cargo")
        .args(["run", "--", "--help"])
        .output()
        .expect("Failed to execute help command");

    assert!(output.status.success(), "Help command should succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Usage: xtrade"), "Should show usage");
    assert!(
        stdout.contains("subscribe"),
        "Should show subscribe command"
    );
    assert!(
        stdout.contains("unsubscribe"),
        "Should show unsubscribe command"
    );
    assert!(stdout.contains("list"), "Should show list command");
    assert!(stdout.contains("ui"), "Should show ui command");
    assert!(stdout.contains("status"), "Should show status command");
    assert!(stdout.contains("show"), "Should show show command");
    assert!(stdout.contains("config"), "Should show config command");
    assert!(stdout.contains("demo"), "Should show demo command");
}

/// Test that the version command works
#[test]
fn test_version_command() {
    let output = Command::new("cargo")
        .args(["run", "--", "--version"])
        .output()
        .expect("Failed to execute version command");

    assert!(output.status.success(), "Version command should succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("xtrade"), "Should show binary name");
    assert!(stdout.contains("0.1.0"), "Should show version number");
}

/// Test config show command
#[test]
fn test_config_show_command() {
    let output = Command::new("cargo")
        .args(["run", "--", "config", "show"])
        .output()
        .expect("Failed to execute config show command");

    assert!(
        output.status.success(),
        "Config show command should succeed"
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Configuration from"),
        "Should show config source"
    );
    assert!(stdout.contains("symbols"), "Should show symbols config");
    assert!(
        stdout.contains("refresh_rate_ms"),
        "Should show refresh rate"
    );
    assert!(
        stdout.contains("orderbook_depth"),
        "Should show orderbook depth"
    );
    assert!(
        stdout.contains("Binance Configuration"),
        "Should show binance config"
    );
    assert!(stdout.contains("UI Configuration"), "Should show UI config");
}

/// Test that subscribe command at least starts (doesn't crash)
#[tokio::test]
async fn test_subscribe_command_starts() {
    let mut child = Command::new("cargo")
        .args(["run", "--", "subscribe", "BTCUSDT"])
        .spawn()
        .expect("Failed to start subscribe command");

    // Wait a bit to ensure it starts properly
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Check if process is still running
    match child.try_wait() {
        Ok(Some(status)) => {
            // Process exited, check if it was successful
            assert!(status.success(), "Subscribe command should exit cleanly");
        }
        Ok(None) => {
            // Process is still running, kill it
            child.kill().expect("Failed to kill subscribe command");
            // This is acceptable - the command might be waiting for data
        }
        Err(e) => panic!("Error checking process status: {}", e),
    }
}

/// Test that list command works
#[test]
fn test_list_command() {
    let output = Command::new("cargo")
        .args(["run", "--", "list"])
        .output()
        .expect("Failed to execute list command");

    assert!(output.status.success(), "List command should succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Currently subscribed symbols"),
        "Should show subscription header"
    );
    assert!(
        stdout.contains("No active subscriptions") || stdout.contains("Total:"),
        "Should show either no subscriptions or subscription count"
    );
}

/// Test that status command works
#[test]
fn test_status_command() {
    let output = Command::new("cargo")
        .args(["run", "--", "status"])
        .output()
        .expect("Failed to execute status command");

    assert!(output.status.success(), "Status command should succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("XTrade Status"),
        "Should show status header"
    );
    assert!(stdout.contains("Version:"), "Should show version");
    assert!(stdout.contains("Status:"), "Should show status");
    assert!(
        stdout.contains("Active subscriptions:"),
        "Should show subscription count"
    );
}

/// Test that show command works
#[test]
fn test_show_command() {
    let output = Command::new("cargo")
        .args(["run", "--", "show", "BTCUSDT"])
        .output()
        .expect("Failed to execute show command");

    assert!(output.status.success(), "Show command should succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Showing details for:"),
        "Should show symbol details header"
    );
    assert!(stdout.contains("BTCUSDT"), "Should show the symbol name");
    assert!(
        stdout.contains("Not subscribed") || stdout.contains("Best bid"),
        "Should show either not subscribed or orderbook details"
    );
}

/// Test that unsubscribe command at least starts (doesn't crash)
#[tokio::test]
async fn test_unsubscribe_command_starts() {
    let mut child = Command::new("cargo")
        .args(["run", "--", "unsubscribe", "BTCUSDT"])
        .spawn()
        .expect("Failed to start unsubscribe command");

    // Wait a bit to ensure it starts properly
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Check if process is still running
    match child.try_wait() {
        Ok(Some(status)) => {
            // Process exited, check if it was successful
            assert!(status.success(), "Unsubscribe command should exit cleanly");
        }
        Ok(None) => {
            // Process is still running, kill it
            child.kill().expect("Failed to kill unsubscribe command");
            // This is acceptable - the command might be processing
        }
        Err(e) => panic!("Error checking process status: {}", e),
    }
}
