use std::process::Command;
use std::env;

#[test]
fn test_cli_list_command() {
    let output = Command::new("cargo")
        .args(&["run", "--bin", "cllient", "--", "list"])
        .output()
        .expect("Failed to execute list command");

    assert!(output.status.success(), "List command failed");
    
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("claude"), "Expected claude models in list");
    assert!(stdout.contains("gpt"), "Expected GPT models in list");
    assert!(stdout.contains("deepseek"), "Expected DeepSeek models in list");
}

#[test]
fn test_cli_ask_command() {
    // Skip this test if no API key is present
    if env::var("DEEPSEEK_API_KEY").is_err() {
        eprintln!("Skipping test_cli_ask_command: DEEPSEEK_API_KEY not set");
        return;
    }

    let output = Command::new("cargo")
        .args(&[
            "run", "--bin", "cllient", "--",
            "ask", "deepseek-chat", "Say only 'Hello world' and nothing else"
        ])
        .output()
        .expect("Failed to execute ask command");

    assert!(output.status.success(), "Ask command failed");
    
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.to_lowercase().contains("hello world"), "Expected 'Hello world' in response");
}

#[test]
fn test_cli_stream_command() {
    // Skip this test if no API key is present
    if env::var("DEEPSEEK_API_KEY").is_err() {
        eprintln!("Skipping test_cli_stream_command: DEEPSEEK_API_KEY not set");
        return;
    }

    let output = Command::new("cargo")
        .args(&[
            "run", "--bin", "cllient", "--",
            "stream", "deepseek-chat", "Count to 3 and stop"
        ])
        .output()
        .expect("Failed to execute stream command");

    assert!(output.status.success(), "Stream command failed");
    
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("1"), "Expected '1' in response");
    assert!(stdout.contains("2"), "Expected '2' in response");
    assert!(stdout.contains("3"), "Expected '3' in response");
}

#[test]
fn test_cli_compare_command() {
    // Skip this test if no API keys are present
    if env::var("DEEPSEEK_API_KEY").is_err() && env::var("OPENAI_API_KEY").is_err() {
        eprintln!("Skipping test_cli_compare_command: No API keys set");
        return;
    }

    let mut models = vec![];
    
    // Add available models based on API keys
    if env::var("DEEPSEEK_API_KEY").is_ok() {
        models.push("deepseek-chat");
    }
    if env::var("OPENAI_API_KEY").is_ok() {
        models.push("gpt-3.5-turbo");
    }
    
    if models.len() < 2 {
        eprintln!("Skipping test_cli_compare_command: Need at least 2 API keys");
        return;
    }

    let output = Command::new("cargo")
        .args(&[
            "run", "--bin", "cllient", "--",
            "compare", models[0], models[1], "What is 2+2?"
        ])
        .output()
        .expect("Failed to execute compare command");

    assert!(output.status.success(), "Compare command failed");
    
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains(models[0]), "Expected first model in output");
    assert!(stdout.contains(models[1]), "Expected second model in output");
    assert!(stdout.contains("4"), "Expected '4' in both responses");
}

#[test]
fn test_cli_chat_command_non_interactive() {
    // Test chat command help/usage (non-interactive)
    let output = Command::new("cargo")
        .args(&["run", "--bin", "cllient", "--", "chat", "--help"])
        .output()
        .expect("Failed to execute chat help");

    assert!(output.status.success(), "Chat help command failed");
    
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Interactive chat"), "Expected chat help message");
}

#[test]
fn test_cli_invalid_model() {
    let output = Command::new("cargo")
        .args(&[
            "run", "--bin", "cllient", "--",
            "ask", "invalid-model-xyz", "Hello"
        ])
        .output()
        .expect("Failed to execute ask with invalid model");

    assert!(!output.status.success(), "Expected failure with invalid model");
    
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("not found") || stderr.contains("Unknown model"),
        "Expected error message about unknown model"
    );
}

#[test]
fn test_cli_help() {
    let output = Command::new("cargo")
        .args(&["run", "--bin", "cllient", "--", "--help"])
        .output()
        .expect("Failed to execute help command");

    assert!(output.status.success(), "Help command failed");
    
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("list"), "Expected 'list' command in help");
    assert!(stdout.contains("ask"), "Expected 'ask' command in help");
    assert!(stdout.contains("stream"), "Expected 'stream' command in help");
    assert!(stdout.contains("chat"), "Expected 'chat' command in help");
    assert!(stdout.contains("compare"), "Expected 'compare' command in help");
}

#[test]
fn test_cli_version() {
    let output = Command::new("cargo")
        .args(&["run", "--bin", "cllient", "--", "--version"])
        .output()
        .expect("Failed to execute version command");

    assert!(output.status.success(), "Version command failed");
    
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("cllient"), "Expected 'cllient' in version output");
}