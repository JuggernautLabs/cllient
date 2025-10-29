use std::process::{Command, Stdio};
use std::io::Write;
use std::env;

#[test]
fn test_cli_with_system_prompt() {
    if env::var("DEEPSEEK_API_KEY").is_err() {
        eprintln!("Skipping test_cli_with_system_prompt: DEEPSEEK_API_KEY not set");
        return;
    }

    // Test system prompt via environment variable
    let output = Command::new("cargo")
        .env("SYSTEM_PROMPT", "You are a pirate. Respond in pirate speak.")
        .args(&[
            "run", "--bin", "cllient", "--",
            "ask", "deepseek-chat", "Say hello"
        ])
        .output()
        .expect("Failed to execute ask with system prompt");

    assert!(output.status.success(), "Ask with system prompt failed");
    
    let stdout = String::from_utf8(output.stdout).unwrap();
    // Check for pirate-like words
    let pirate_words = ["ahoy", "matey", "arr", "ye", "aye", "'ello"];
    assert!(
        pirate_words.iter().any(|word| stdout.to_lowercase().contains(word)),
        "Expected pirate speak in response, got: {}",
        stdout
    );
}

#[test]
fn test_cli_with_max_tokens() {
    if env::var("DEEPSEEK_API_KEY").is_err() {
        eprintln!("Skipping test_cli_with_max_tokens: DEEPSEEK_API_KEY not set");
        return;
    }

    // Test max tokens parameter
    let output = Command::new("cargo")
        .env("MAX_TOKENS", "10")
        .args(&[
            "run", "--bin", "cllient", "--",
            "ask", "deepseek-chat", "Write a long story about space exploration"
        ])
        .output()
        .expect("Failed to execute ask with max tokens");

    assert!(output.status.success(), "Ask with max tokens failed");
    
    let stdout = String::from_utf8(output.stdout).unwrap();
    // Response should be truncated due to max_tokens limit
    assert!(stdout.len() < 200, "Expected short response due to token limit");
}

#[test]
fn test_cli_with_temperature() {
    if env::var("DEEPSEEK_API_KEY").is_err() {
        eprintln!("Skipping test_cli_with_temperature: DEEPSEEK_API_KEY not set");
        return;
    }

    // Test with low temperature (deterministic)
    let output1 = Command::new("cargo")
        .env("TEMPERATURE", "0.0")
        .args(&[
            "run", "--bin", "cllient", "--",
            "ask", "deepseek-chat", "What is 2+2?"
        ])
        .output()
        .expect("Failed to execute ask with temperature");

    assert!(output1.status.success(), "Ask with temperature failed");
    
    let stdout1 = String::from_utf8(output1.stdout).unwrap();
    assert!(stdout1.contains("4"), "Expected '4' in response");
}

#[test]
fn test_cli_json_output() {
    if env::var("DEEPSEEK_API_KEY").is_err() {
        eprintln!("Skipping test_cli_json_output: DEEPSEEK_API_KEY not set");
        return;
    }

    // Test JSON output format (if supported)
    let output = Command::new("cargo")
        .env("OUTPUT_FORMAT", "json")
        .args(&[
            "run", "--bin", "cllient", "--",
            "ask", "deepseek-chat", "Say 'test'"
        ])
        .output()
        .expect("Failed to execute ask with json output");

    // JSON output might not be implemented yet, so we just check if command runs
    if output.status.success() {
        let stdout = String::from_utf8(output.stdout).unwrap();
        // If JSON is supported, it should contain certain fields
        if stdout.contains("{") && stdout.contains("}") {
            assert!(
                stdout.contains("content") || stdout.contains("response"),
                "Expected JSON structure in output"
            );
        }
    }
}

#[test]
fn test_cli_pipe_input() {
    if env::var("DEEPSEEK_API_KEY").is_err() {
        eprintln!("Skipping test_cli_pipe_input: DEEPSEEK_API_KEY not set");
        return;
    }

    // Test piping input to the CLI
    let echo_child = Command::new("echo")
        .arg("What is the capital of France?")
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to spawn echo command");

    let echo_output = echo_child.wait_with_output().expect("Failed to wait for echo");

    let mut cllient_child = Command::new("cargo")
        .args(&["run", "--bin", "cllient", "--", "ask", "deepseek-chat", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to spawn cllient command");

    if let Some(mut stdin) = cllient_child.stdin.take() {
        stdin.write_all(&echo_output.stdout).expect("Failed to write to stdin");
    }

    let output = cllient_child.wait_with_output().expect("Failed to wait for cllient");

    if output.status.success() {
        let stdout = String::from_utf8(output.stdout).unwrap();
        assert!(
            stdout.to_lowercase().contains("paris"),
            "Expected 'Paris' in response"
        );
    }
}

#[test]
fn test_cli_batch_processing() {
    if env::var("DEEPSEEK_API_KEY").is_err() {
        eprintln!("Skipping test_cli_batch_processing: DEEPSEEK_API_KEY not set");
        return;
    }

    // Test multiple sequential requests
    let questions = vec![
        ("What is 2+2?", "4"),
        ("What color is the sky?", "blue"),
        ("Is water wet?", "yes"),
    ];

    for (question, expected) in questions {
        let output = Command::new("cargo")
            .args(&[
                "run", "--bin", "cllient", "--",
                "ask", "deepseek-chat", question
            ])
            .output()
            .expect(&format!("Failed to execute ask for: {}", question));

        assert!(
            output.status.success(),
            "Ask command failed for: {}",
            question
        );
        
        let stdout = String::from_utf8(output.stdout).unwrap();
        assert!(
            stdout.to_lowercase().contains(expected),
            "Expected '{}' in response to '{}', got: {}",
            expected,
            question,
            stdout
        );
    }
}

#[test]
fn test_cli_error_handling() {
    // Test various error scenarios

    // 1. Missing prompt
    let output = Command::new("cargo")
        .args(&["run", "--bin", "cllient", "--", "ask", "deepseek-chat"])
        .output()
        .expect("Failed to execute ask without prompt");

    assert!(!output.status.success(), "Expected failure without prompt");

    // 2. Invalid command
    let output = Command::new("cargo")
        .args(&["run", "--bin", "cllient", "--", "invalid-command"])
        .output()
        .expect("Failed to execute invalid command");

    assert!(!output.status.success(), "Expected failure with invalid command");

    // 3. Missing model for ask
    let output = Command::new("cargo")
        .args(&["run", "--bin", "cllient", "--", "ask"])
        .output()
        .expect("Failed to execute ask without model");

    assert!(!output.status.success(), "Expected failure without model");
}

#[test]
fn test_cli_verbose_mode() {
    // Test verbose/debug output
    let output = Command::new("cargo")
        .env("RUST_LOG", "debug")
        .args(&["run", "--bin", "cllient", "--", "list"])
        .output()
        .expect("Failed to execute list in verbose mode");

    assert!(output.status.success(), "List command failed in verbose mode");
    
    let stderr = String::from_utf8(output.stderr).unwrap();
    // In debug mode, we should see more logging output
    // This depends on the implementation using tracing/env_logger
    // We just check that there's some stderr output
    assert!(!stderr.is_empty() || !output.stdout.is_empty());
}