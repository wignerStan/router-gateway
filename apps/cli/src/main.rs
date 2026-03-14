//! Gateway CLI - Management utility for the LLM Gateway
//!
//! This CLI provides commands for managing the gateway, including:
//! - Health checks
//! - Model listing
//! - Configuration validation

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tabled::{Table, Tabled};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Commands,

    /// Output format (text, json)
    #[arg(short, long, global = true, default_value = "text")]
    format: String,

    /// Verbose output
    #[arg(short, long, global = true)]
    verbose: bool,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Check gateway health status
    Health {
        /// Gateway URL
        #[arg(short, long, default_value = "http://localhost:3000")]
        url: String,
    },

    /// List available models
    Models {
        /// Gateway URL
        #[arg(short, long, default_value = "http://localhost:3000")]
        url: String,
    },

    /// Validate configuration file
    Validate {
        /// Path to configuration file
        #[arg(short, long)]
        config: PathBuf,
    },
}

/// Health status response from gateway
#[derive(Debug, Serialize, Deserialize)]
struct HealthStatus {
    status: String,
    uptime_secs: u64,
    credential_count: usize,
    healthy_count: usize,
    degraded_count: usize,
    unhealthy_count: usize,
}

/// Model info from gateway
#[derive(Debug, Serialize, Deserialize)]
struct ModelInfo {
    id: String,
    provider: String,
    #[serde(default)]
    capabilities: Vec<String>,
    #[serde(default)]
    context_window: usize,
}

/// Models list response from gateway
#[derive(Debug, Serialize, Deserialize)]
struct ModelsListResponse {
    models: Vec<ModelInfo>,
    count: usize,
    #[serde(default)]
    message: Option<String>,
}

/// Parsed gateway configuration for validation
#[derive(Debug, Deserialize)]
struct GatewayConfigYaml {
    #[serde(default)]
    server: Option<ServerConfigYaml>,
    #[serde(default)]
    credentials: Vec<CredentialConfigYaml>,
    #[serde(default)]
    routing: Option<RoutingConfigYaml>,
}

#[derive(Debug, Deserialize)]
struct ServerConfigYaml {
    #[serde(default = "default_port")]
    port: u16,
    #[serde(default)]
    host: Option<String>,
    #[serde(default)]
    timeout_secs: Option<u64>,
}

fn default_port() -> u16 {
    3000
}

#[derive(Debug, Deserialize)]
struct CredentialConfigYaml {
    id: String,
    provider: String,
    api_key: String,
    #[serde(default)]
    base_url: Option<String>,
    #[serde(default)]
    allowed_models: Vec<String>,
    #[serde(default)]
    priority: i32,
}

#[derive(Debug, Deserialize)]
struct RoutingConfigYaml {
    #[serde(default = "default_strategy")]
    strategy: String,
    #[serde(default = "default_true")]
    session_affinity: bool,
}

fn default_strategy() -> String {
    "weighted".to_string()
}

fn default_true() -> bool {
    true
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let args = Args::parse();
    let format_json = args.format == "json";

    match args.command {
        Commands::Health { url } => {
            let health_url = format!("{}/health", url);
            let start = std::time::Instant::now();

            let response = reqwest::get(&health_url)
                .await
                .context("Failed to connect to gateway")?;

            let latency_ms = start.elapsed().as_millis();

            if !response.status().is_success() {
                if format_json {
                    println!(
                        "{{\"status\": \"error\", \"code\": {}, \"latency_ms\": {}}}",
                        response.status().as_u16(),
                        latency_ms
                    );
                } else {
                    println!(
                        "{}: Gateway returned status {}",
                        "UNHEALTHY".red().bold(),
                        response.status()
                    );
                }
                anyhow::bail!("UNHEALTHY: Gateway returned status {}", response.status());
            }

            let health: HealthStatus = response
                .json()
                .await
                .context("Failed to parse health response")?;

            if format_json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "status": health.status,
                        "uptime_secs": health.uptime_secs,
                        "latency_ms": latency_ms,
                        "credential_count": health.credential_count,
                        "healthy_count": health.healthy_count,
                        "degraded_count": health.degraded_count,
                        "unhealthy_count": health.unhealthy_count,
                    }))?
                );
            } else {
                println!("{}", "GATEWAY HEALTH".green().bold());
                println!("{}", "=".repeat(50));
                println!("Status:           {}", health.status.green());
                println!("Uptime:           {}s", health.uptime_secs);
                println!("Latency:          {}ms", latency_ms);
                println!();
                println!("Credentials:");
                println!("  Total:          {}", health.credential_count);
                println!(
                    "  Healthy:        {}",
                    health.healthy_count.to_string().green()
                );
                if health.degraded_count > 0 {
                    println!(
                        "  Degraded:       {}",
                        health.degraded_count.to_string().yellow()
                    );
                }
                if health.unhealthy_count > 0 {
                    println!(
                        "  Unhealthy:      {}",
                        health.unhealthy_count.to_string().red()
                    );
                }
            }

            // Exit with code based on health
            if health.status != "healthy" || health.unhealthy_count > 0 {
                anyhow::bail!(
                    "UNHEALTHY: status={}, unhealthy={}",
                    health.status,
                    health.unhealthy_count
                );
            }
        },

        Commands::Models { url } => {
            let models_url = format!("{}/api/models", url);

            let response = reqwest::get(&models_url)
                .await
                .context("Failed to connect to gateway")?;

            if !response.status().is_success() {
                anyhow::bail!("Gateway returned status {}", response.status());
            }

            let models: ModelsListResponse = response
                .json()
                .await
                .context("Failed to parse models response")?;

            if format_json {
                println!("{}", serde_json::to_string_pretty(&models)?);
            } else if models.models.is_empty() {
                println!("No models configured.");
                if let Some(msg) = models.message {
                    println!("{}", msg.yellow());
                }
            } else {
                #[derive(Tabled)]
                struct ModelRow {
                    #[tabled(rename = "ID")]
                    id: String,
                    #[tabled(rename = "Provider")]
                    provider: String,
                    #[tabled(rename = "Context")]
                    context: String,
                    #[tabled(rename = "Capabilities")]
                    capabilities: String,
                }

                let rows: Vec<ModelRow> = models
                    .models
                    .iter()
                    .map(|m| ModelRow {
                        id: m.id.clone(),
                        provider: m.provider.clone(),
                        context: format!("{}K", m.context_window / 1000),
                        capabilities: m.capabilities.join(", "),
                    })
                    .collect();

                let table = Table::new(rows);
                println!("{}", table);
                println!();
                println!("Total: {} models", models.count);
            }
        },

        Commands::Validate { config } => {
            if !config.exists() {
                if format_json {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&serde_json::json!({
                            "valid": false,
                            "error": format!("File not found: {}", config.display())
                        }))?
                    );
                } else {
                    println!(
                        "{}: File not found: {}",
                        "ERROR".red().bold(),
                        config.display()
                    );
                }
                anyhow::bail!("File not found: {}", config.display());
            }

            // Read and parse the config file
            // Read file content
            let content =
                std::fs::read_to_string(&config).context("Failed to read configuration file")?;

            // Parse as YAML and validate structure
            let validation_result = validate_config_content(&content);

            match validation_result {
                Ok((config_value, errors, warnings)) => {
                    if format_json {
                        let server_info = config_value.server.as_ref().map(|s| {
                            serde_json::json!({
                                "port": s.port,
                                "host": s.host,
                                "timeout_secs": s.timeout_secs,
                            })
                        });
                        println!(
                            "{}",
                            serde_json::to_string_pretty(&serde_json::json!({
                                "valid": errors.is_empty(),
                                "errors": errors,
                                "warnings": warnings,
                                "file": config.display().to_string(),
                                "credentials_count": config_value.credentials.len(),
                                "server": server_info,
                                "routing": config_value.routing.as_ref().map(|r| &r.strategy),
                            }))?
                        );
                    } else if errors.is_empty() {
                        println!("{} Configuration is valid", "✓".green());
                        println!("  File: {}", config.display());
                        if let Some(ref server) = config_value.server {
                            println!(
                                "  Server: port={}, host={}",
                                server.port,
                                server.host.as_deref().unwrap_or("default")
                            );
                        }
                        println!("  Credentials: {} defined", config_value.credentials.len());
                        for cred in &config_value.credentials {
                            println!(
                                "    - {} ({}): priority={}, models={}",
                                cred.id,
                                cred.provider,
                                cred.priority,
                                cred.allowed_models.len()
                            );
                            if let Some(ref base_url) = cred.base_url {
                                println!("      base_url: {}", base_url);
                            }
                        }
                        if let Some(ref routing) = config_value.routing {
                            println!(
                                "  Strategy: {} (session_affinity={})",
                                routing.strategy, routing.session_affinity
                            );
                        }
                        if !warnings.is_empty() {
                            println!();
                            println!("{}", "Warnings:".yellow());
                            for warning in &warnings {
                                println!("  ⚠ {}", warning);
                            }
                        }
                    } else {
                        println!("{} Configuration has errors", "✗".red());
                        println!();
                        for error in &errors {
                            println!("  ✗ {}", error);
                        }
                    }

                    if !errors.is_empty() {
                        anyhow::bail!(
                            "Configuration validation failed with {} errors",
                            errors.len()
                        );
                    }
                },
                Err(e) => {
                    if format_json {
                        println!(
                            "{}",
                            serde_json::to_string_pretty(&serde_json::json!({
                                "valid": false,
                                "error": e.to_string()
                            }))?
                        );
                    } else {
                        println!("{}: Invalid YAML syntax", "ERROR".red().bold());
                        println!("  {}", e);
                    }
                    anyhow::bail!("Invalid YAML syntax: {}", e);
                },
            }
        },
    }

    Ok(())
}

/// Validates the content of a gateway configuration file
fn validate_config_content(
    content: &str,
) -> Result<(GatewayConfigYaml, Vec<String>, Vec<String>), serde_yaml::Error> {
    let config_value: GatewayConfigYaml = serde_yaml::from_str(content)?;
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    // Validate credentials
    for (i, cred) in config_value.credentials.iter().enumerate() {
        // Check required fields
        if cred.id.is_empty() {
            errors.push(format!("Credential {}: 'id' field is empty", i));
        }
        if cred.provider.is_empty() {
            errors.push(format!("Credential {}: 'provider' field is empty", i));
        }
        if cred.api_key.is_empty() {
            errors.push(format!("Credential {}: 'api_key' field is empty", i));
        }

        // Check for env var in api_key
        if cred.api_key.starts_with("${") && !cred.api_key.contains(":-") {
            let var_name = cred.api_key.trim_start_matches("${").trim_end_matches("}");
            if std::env::var(var_name).is_err() {
                warnings.push(format!(
                    "Credential '{}': environment variable '{}' not set",
                    cred.id, var_name
                ));
            }
        }
    }

    // Validate routing strategy
    if let Some(ref routing) = config_value.routing {
        let valid_strategies = [
            "weighted",
            "adaptive",
            "round_robin",
            "time_aware",
            "quota_aware",
            "policy_aware",
        ];
        if !valid_strategies.contains(&routing.strategy.as_str()) {
            errors.push(format!(
                "Invalid routing strategy: '{}'. Valid options: {:?}",
                routing.strategy, valid_strategies
            ));
        }
    }

    Ok((config_value, errors, warnings))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_valid_config() {
        let yaml = r#"
server:
  port: 8080
credentials:
  - id: openai-1
    provider: openai
    api_key: "sk-..."
    priority: 1
    allowed_models: ["gpt-4"]
routing:
  strategy: adaptive
"#;
        let result = validate_config_content(yaml);
        assert!(result.is_ok());
        let (_, errors, _) = result.expect("value must be present");
        assert!(errors.is_empty());
    }

    #[test]
    fn test_validate_invalid_strategy() {
        let yaml = r#"
credentials:
  - id: test
    provider: openai
    api_key: "key"
routing:
  strategy: invalid_strategy
"#;
        let result = validate_config_content(yaml);
        assert!(result.is_ok());
        let (_, errors, _) = result.expect("value must be present");
        assert!(!errors.is_empty());
        assert!(errors[0].contains("Invalid routing strategy"));
    }

    #[test]
    fn test_validate_empty_fields() {
        let yaml = r#"
credentials:
  - id: ""
    provider: ""
    api_key: ""
"#;
        let result = validate_config_content(yaml);
        assert!(result.is_ok());
        let (_, errors, _) = result.expect("value must be present");
        assert_eq!(errors.len(), 3);
        assert!(errors[0].contains("'id' field is empty"));
        assert!(errors[1].contains("'provider' field is empty"));
        assert!(errors[2].contains("'api_key' field is empty"));
    }

    #[test]
    fn test_validate_invalid_yaml() {
        let yaml = "invalid: yaml: :";
        let result = validate_config_content(yaml);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_missing_env_var_warning() {
        let yaml = r#"
credentials:
  - id: test
    provider: openai
    api_key: "${NON_EXISTENT_VAR_123}"
"#;
        let result = validate_config_content(yaml);
        assert!(result.is_ok());
        let (_, _, warnings) = result.expect("value must be present");
        assert!(!warnings.is_empty());
        assert!(warnings[0].contains("environment variable 'NON_EXISTENT_VAR_123' not set"));
    }
}
