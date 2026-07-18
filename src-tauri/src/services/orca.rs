use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::Value;

use crate::models::{OrcaInventory, OrcaTopic};

const ORCA_COMMAND_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug)]
enum OrcaCommandError {
    Missing,
    Failed(String),
    TimedOut,
    InvalidJson(String),
}

pub struct OrcaService;

impl OrcaService {
    pub fn inspect() -> OrcaInventory {
        let checked_at = current_timestamp();
        let status = match run_json(&["status", "--json"]) {
            Ok(value) => value,
            Err(OrcaCommandError::Missing) => {
                return unavailable(
                    checked_at,
                    false,
                    "Orca CLI was not found on PATH".to_string(),
                )
            }
            Err(error) => {
                return unavailable(checked_at, true, format_command_error("status", error))
            }
        };

        let (app_running, runtime_reachable, runtime_state) = parse_status(&status);
        let available = runtime_reachable
            .unwrap_or_else(|| status.get("ok").and_then(Value::as_bool).unwrap_or(false));

        let (topics_available, topics, topic_warning) =
            match run_json(&["skills", "list", "--json"]) {
                Ok(value) => match parse_topics(&value) {
                    Some(topics) => (true, topics, None),
                    None => (
                        false,
                        Vec::new(),
                        Some("Orca returned an unexpected skills list shape".to_string()),
                    ),
                },
                Err(error) => (
                    false,
                    Vec::new(),
                    Some(format_command_error("skills list", error)),
                ),
            };

        OrcaInventory {
            cli_available: true,
            available,
            app_running,
            runtime_reachable,
            runtime_state,
            topics_available,
            topics,
            checked_at,
            warning: topic_warning,
        }
    }
}

fn run_json(args: &[&str]) -> Result<Value, OrcaCommandError> {
    let mut command = Command::new("orca");
    command
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }

    let mut child = command.spawn().map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            OrcaCommandError::Missing
        } else {
            OrcaCommandError::Failed(error.to_string())
        }
    })?;
    let started = Instant::now();

    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if started.elapsed() >= ORCA_COMMAND_TIMEOUT => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(OrcaCommandError::TimedOut);
            }
            Ok(None) => thread::sleep(Duration::from_millis(25)),
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(OrcaCommandError::Failed(error.to_string()));
            }
        }
    }

    let output = child
        .wait_with_output()
        .map_err(|error| OrcaCommandError::Failed(error.to_string()))?;
    if !output.status.success() {
        let message = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(OrcaCommandError::Failed(if message.is_empty() {
            format!("process exited with {}", output.status)
        } else {
            message
        }));
    }

    serde_json::from_slice(&output.stdout)
        .map_err(|error| OrcaCommandError::InvalidJson(error.to_string()))
}

fn parse_status(value: &Value) -> (Option<bool>, Option<bool>, Option<String>) {
    let result = value.get("result").unwrap_or(value);
    let app = result.get("app").unwrap_or(&Value::Null);
    let runtime = result.get("runtime").unwrap_or(&Value::Null);
    (
        app.get("running").and_then(Value::as_bool),
        runtime
            .get("reachable")
            .and_then(Value::as_bool)
            .or_else(|| value.get("ok").and_then(Value::as_bool)),
        runtime
            .get("state")
            .and_then(Value::as_str)
            .map(ToString::to_string),
    )
}

fn parse_topics(value: &Value) -> Option<Vec<OrcaTopic>> {
    let topics = value
        .get("topics")
        .or_else(|| value.get("result").and_then(|result| result.get("topics")))?
        .as_array()?;

    Some(
        topics
            .iter()
            .filter_map(|topic| {
                let name = topic
                    .get("name")
                    .or_else(|| topic.get("id"))
                    .or_else(|| topic.get("title"))
                    .and_then(Value::as_str)?
                    .trim();
                if name.is_empty() {
                    return None;
                }
                Some(OrcaTopic {
                    name: name.to_string(),
                    description: topic
                        .get("description")
                        .and_then(Value::as_str)
                        .map(ToString::to_string),
                })
            })
            .collect(),
    )
}

fn unavailable(checked_at: u64, cli_available: bool, warning: String) -> OrcaInventory {
    OrcaInventory {
        cli_available,
        available: false,
        app_running: None,
        runtime_reachable: None,
        runtime_state: None,
        topics_available: false,
        topics: Vec::new(),
        checked_at,
        warning: Some(warning),
    }
}

fn format_command_error(command: &str, error: OrcaCommandError) -> String {
    match error {
        OrcaCommandError::Missing => format!("Orca CLI was not found while running {command}"),
        OrcaCommandError::Failed(message) => format!("Orca {command} failed: {message}"),
        OrcaCommandError::TimedOut => format!("Orca {command} timed out after 5 seconds"),
        OrcaCommandError::InvalidJson(message) => {
            format!("Orca {command} returned invalid JSON: {message}")
        }
    }
}

fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::{parse_status, parse_topics, unavailable};
    use serde_json::json;

    #[test]
    fn parses_orca_status_result_shape() {
        let value = json!({
            "ok": true,
            "result": {
                "app": {"running": true},
                "runtime": {"reachable": true, "state": "ready"}
            }
        });

        assert_eq!(
            parse_status(&value),
            (Some(true), Some(true), Some("ready".to_string()))
        );
    }

    #[test]
    fn parses_orca_topics_and_ignores_invalid_entries() {
        let value = json!({
            "topics": [
                {"name": "computer-use", "description": "Desktop control"},
                {"id": "orca-cli"},
                {"description": "missing name"}
            ]
        });

        let topics = parse_topics(&value).expect("topics should parse");
        assert_eq!(topics.len(), 2);
        assert_eq!(topics[0].name, "computer-use");
        assert_eq!(topics[1].name, "orca-cli");
    }

    #[test]
    fn rejects_malformed_topic_shape() {
        assert!(parse_topics(&json!({"topics": {}})).is_none());
    }

    #[test]
    fn preserves_empty_topic_inventory() {
        let topics = parse_topics(&json!({"topics": []})).expect("empty topics should parse");
        assert!(topics.is_empty());
    }

    #[test]
    fn represents_offline_runtime_without_hiding_cli_presence() {
        let value = json!({
            "ok": true,
            "result": {
                "app": {"running": false},
                "runtime": {"reachable": false, "state": "offline"}
            }
        });

        let (app_running, reachable, state) = parse_status(&value);
        assert_eq!(app_running, Some(false));
        assert_eq!(reachable, Some(false));
        assert_eq!(state.as_deref(), Some("offline"));
    }

    #[test]
    fn represents_missing_cli_as_unavailable() {
        let inventory = unavailable(42, false, "missing".to_string());
        assert!(!inventory.cli_available);
        assert!(!inventory.available);
        assert!(!inventory.topics_available);
        assert!(inventory.topics.is_empty());
    }
}
