use assert_cmd::Command;
use std::io::Write;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn write_temp_file(content: &str) -> tempfile::NamedTempFile {
    let mut file = tempfile::NamedTempFile::new().expect("failed to create temp file");
    file.write_all(content.as_bytes())
        .expect("failed to write temp file");
    file
}

// -- validate command tests --

#[test]
fn validate_with_valid_config_exits_zero() {
    let yaml = r#"
server:
  port: 8080
credentials:
  - id: openai-1
    provider: openai
    api_key: "sk-test123"
    priority: 1
    allowed_models: ["gpt-4"]
"#;
    let tmp = write_temp_file(yaml);
    Command::cargo_bin("my-cli")
        .expect("my-cli binary should exist")
        .arg("validate")
        .arg("-c")
        .arg(tmp.path())
        .assert()
        .success();
}

#[test]
fn validate_with_invalid_yaml_exits_nonzero() {
    let tmp = write_temp_file("invalid: yaml: :");
    Command::cargo_bin("my-cli")
        .expect("my-cli binary should exist")
        .arg("validate")
        .arg("-c")
        .arg(tmp.path())
        .assert()
        .failure()
        .stderr(predicates::str::contains("Invalid YAML syntax"));
}

#[test]
fn validate_with_nonexistent_file_exits_nonzero() {
    let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
    let nonexistent_path = temp_dir.path().join("nonexistent_config.yaml");
    Command::cargo_bin("my-cli")
        .expect("my-cli binary should exist")
        .arg("validate")
        .arg("-c")
        .arg(nonexistent_path)
        .assert()
        .failure()
        .stderr(predicates::str::contains("File not found"));
}

#[test]
fn validate_format_json_produces_valid_json() {
    let yaml = r#"
credentials:
  - id: test-cred
    provider: openai
    api_key: "sk-test"
"#;
    let tmp = write_temp_file(yaml);
    Command::cargo_bin("my-cli")
        .expect("my-cli binary should exist")
        .arg("validate")
        .arg("-c")
        .arg(tmp.path())
        .arg("-f")
        .arg("json")
        .assert()
        .success()
        .stdout(predicates::str::is_match(r#""valid"\s*:\s*true"#).expect("value must be present"));
}

#[test]
fn verbose_flag_does_not_crash() {
    let yaml = r#"
credentials:
  - id: test
    provider: openai
    api_key: "sk-test"
"#;
    let tmp = write_temp_file(yaml);
    Command::cargo_bin("my-cli")
        .expect("my-cli binary should exist")
        .arg("validate")
        .arg("-c")
        .arg(tmp.path())
        .arg("--verbose")
        .assert()
        .success();
}

// -- health command tests (using wiremock) --

#[tokio::test]
async fn health_against_test_server_returns_status() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/health"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status": "healthy",
            "uptime_secs": 42,
            "credential_count": 1,
            "healthy_count": 1,
            "degraded_count": 0,
            "unhealthy_count": 0
        })))
        .mount(&server)
        .await;

    let url = server.uri();
    Command::cargo_bin("my-cli")
        .expect("my-cli binary should exist")
        .arg("health")
        .arg("--url")
        .arg(&url)
        .assert()
        .success()
        .stdout(predicates::str::contains("healthy"));
}

// -- models command tests (using wiremock) --

#[tokio::test]
async fn models_against_test_server_lists_models() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "models": [
                {
                    "id": "gpt-4",
                    "provider": "openai",
                    "capabilities": ["chat"],
                    "context_window": 128_000
                }
            ],
            "count": 1
        })))
        .mount(&server)
        .await;

    let url = server.uri();
    Command::cargo_bin("my-cli")
        .expect("my-cli binary should exist")
        .arg("models")
        .arg("--url")
        .arg(&url)
        .assert()
        .success()
        .stdout(predicates::str::contains("gpt-4"));
}

#[tokio::test]
async fn models_format_json_produces_valid_json() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "models": [],
            "count": 0,
            "message": null
        })))
        .mount(&server)
        .await;

    let url = server.uri();
    let assert = Command::cargo_bin("my-cli")
        .expect("my-cli binary should exist")
        .arg("models")
        .arg("--url")
        .arg(&url)
        .arg("-f")
        .arg("json")
        .assert()
        .success();

    let output = assert.get_output();
    let actual: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("stdout should be valid JSON");
    let expected = serde_json::json!({
        "models": [],
        "count": 0,
        "message": null
    });
    assert_eq!(actual, expected);
}

// -- connection failure test --

#[tokio::test]
async fn health_connection_failure_reports_error() {
    Command::cargo_bin("my-cli")
        .expect("my-cli binary should exist")
        .arg("health")
        .arg("--url")
        .arg("http://127.0.0.1:1")
        .assert()
        .failure();
}
