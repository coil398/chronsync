use cron::Schedule;
use serde::{Deserialize, Deserializer};
use std::error::Error;
use std::path::Path;
use std::str::FromStr;

fn deserialize_schedule<'de, D>(deserializer: D) -> Result<Schedule, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;

    Schedule::from_str(&s).map_err(serde::de::Error::custom)
}

#[derive(Debug, Deserialize, Clone)]
pub struct Task {
    pub name: String,

    #[serde(deserialize_with = "deserialize_schedule")]
    pub cron_schedule: Schedule,

    pub command: String,
    pub args: Option<Vec<String>>,

    #[serde(default)]
    pub timeout: Option<u64>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub tasks: Vec<Task>,
}

pub fn load_config(path: &Path) -> Result<Config, Box<dyn Error>> {
    use std::fs;

    let content = fs::read_to_string(path)?;
    let config: Config = serde_json::from_str(&content)?;

    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_config_deserialization() {
        let json_data = r#"
        {
            "tasks": [
                {
                    "name": "test_task",
                    "cron_schedule": "*/5 * * * * *",
                    "command": "echo",
                    "args": ["hello"],
                    "timeout": 10
                }
            ]
        }"#;

        let config: Config =
            serde_json::from_str(json_data).expect("Should deserialize valid config");
        assert_eq!(config.tasks.len(), 1);
        assert_eq!(config.tasks[0].name, "test_task");
        assert_eq!(config.tasks[0].timeout, Some(10));
    }

    #[test]
    fn test_invalid_cron_schedule() {
        let json_data = r#"
        {
            "tasks": [
                {
                    "name": "bad_cron",
                    "cron_schedule": "INVALID_CRON_STRING",
                    "command": "echo"
                }
            ]
        }"#;

        let result: Result<Config, _> = serde_json::from_str(json_data);
        assert!(result.is_err(), "Should fail on invalid cron schedule");
    }

    #[test]
    fn test_missing_command_field() {
        let json_data = r#"
        {
            "tasks": [
                {
                    "name": "missing_command",
                    "cron_schedule": "* * * * * *"
                }
            ]
        }"#;

        let result: Result<Config, _> = serde_json::from_str(json_data);
        assert!(
            result.is_err(),
            "Should fail when mandatory field 'command' is missing"
        );
    }
}
