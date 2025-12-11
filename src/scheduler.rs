use crate::config::{Config, Task};
use cron::Schedule;
use log::{error, info, warn};
use reqwest::Client;
use serde_json::json;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::process::Command;
use tokio::task::JoinHandle;
use tokio::time::{self, sleep, Duration};

type JobHandle = Arc<Mutex<Option<JoinHandle<()>>>>;

pub struct TaskScheduler {
    job_handles: Vec<JobHandle>,
}

impl TaskScheduler {
    pub fn new() -> Self {
        TaskScheduler {
            job_handles: Vec::new(),
        }
    }

    pub fn reload_tasks(&mut self, config: Config) {
        info!(
            "[Scheduler] Stopping {} existing tasks...",
            self.job_handles.len()
        );

        for handle_mutex in self.job_handles.drain(..) {
            if let Some(handle) = handle_mutex.lock().unwrap().take() {
                handle.abort();
            }
        }

        info!(
            "[Scheduler] Existing tasks stopped. Registering {} new tasks...",
            config.tasks.len()
        );

        for task in config.tasks {
            self.register_task(task);
        }
    }

    fn register_task(&mut self, task: Task) {
        let schedule = task.cron_schedule.clone();
        let name = task.name.clone();

        let handle_ref: JobHandle = Arc::new(Mutex::new(None));

        let handle_ref_for_job = handle_ref.clone();

        self.job_handles.push(handle_ref.clone());

        info!(
            "[Scheduler] Registering task '{}' with schedule: {}",
            name, schedule
        );

        let job_task = tokio::spawn(async move {
            TaskScheduler::run_job_loop(
                name,
                schedule,
                task.command,
                task.args,
                task.timeout,
                task.webhook_url,
                task.cwd,
                task.env,
            )
            .await;

            handle_ref_for_job.lock().unwrap().take();
        });

        *handle_ref.lock().unwrap() = Some(job_task);
    }

    async fn run_job_loop(
        name: String,
        schedule: Schedule,
        command: String,
        args: Option<Vec<String>>,
        timeout: Option<u64>,
        webhook_url: Option<String>,
        cwd: Option<String>,
        env: Option<HashMap<String, String>>,
    ) {
        let mut job_running = true;

        while job_running {
            let now = chrono::Local::now();

            if let Some(next_execution) = schedule.upcoming(chrono::Local).next() {
                let delay = next_execution - now;
                let duration = delay.to_std().unwrap_or_default();

                sleep(duration).await;

                TaskScheduler::execute_command(
                    &name,
                    &command,
                    args.as_deref().unwrap_or(&[]),
                    timeout,
                    webhook_url.as_deref(),
                    cwd.as_deref(),
                    env.as_ref(),
                )
                .await;
            } else {
                warn!(
                    "[{}] Schedule ended or failed to calculate next time.",
                    name
                );
                job_running = false;
            }
        }
    }

    pub async fn execute_command(
        name: &str,
        command: &str,
        args: &[String],
        timeout: Option<u64>,
        webhook_url: Option<&str>,
        cwd: Option<&str>,
        env: Option<&HashMap<String, String>>,
    ) {
        info!("[{}] -> Command starting: {} {:?}", name, command, args);

        let mut cmd_to_run = Command::new(command);
        cmd_to_run.args(args);

        if let Some(dir) = cwd {
            cmd_to_run.current_dir(dir);
            info!("[{}] CWD set to: {}", name, dir);
        }

        if let Some(envs) = env {
            cmd_to_run.envs(envs);
            let keys: Vec<&str> = envs.keys().map(|k| k.as_str()).collect();
            info!("[{}] Envs set: {:?}", name, keys);
        }

        let child = match cmd_to_run.spawn() {
            Ok(c) => c,
            Err(e) => {
                error!("[{}] -> Failed to spawn command '{}': {}", name, command, e);
                return;
            }
        };
        let child_pid = child.id();

        let execution_future = child.wait_with_output();

        let output_result = if let Some(s) = timeout {
            info!("[{}] Running command with timeout: {}s", name, s);

            let duration = Duration::from_secs(s);

            match time::timeout(duration, execution_future).await {
                Ok(result) => result,
                Err(_) => {
                    error!(
                        "[{}] -> Command TIMEOUT after {} seconds. Killing process.",
                        name, s
                    );

                    if let Some(pid) = child_pid {
                        let kill_status = tokio::process::Command::new("kill")
                            .arg("-9")
                            .arg(pid.to_string())
                            .status()
                            .await;

                        match kill_status {
                            Ok(status) if status.success() => {
                                error!("[{}] Child process PID {} killed successfully.", name, pid);
                            }
                            _ => {
                                error!("[{}] Failed to kill child process PID {}.", name, pid);
                            }
                        }
                    }

                    let io_error = std::io::Error::new(
                        std::io::ErrorKind::TimedOut,
                        format!("Command timed out after {} seconds", s),
                    );

                    Err(io_error)
                }
            }
        } else {
            info!("[{}] Running command (no timeout limit)", name);
            execution_future.await
        };

        match output_result {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);

                if output.status.success() {
                    info!("[{}] -> Command SUCCESS. Status: {}", name, output.status);
                    if !stdout.trim().is_empty() {
                        info!("[{}] -> STDOUT:\n{}", name, stdout.trim());
                    }
                } else {
                    error!("[{}] -> Command FAILED. Status: {}", name, output.status);
                    if !stderr.trim().is_empty() {
                        error!("[{}] -> STDERR:\n{}", name, stderr.trim());
                    }

                    if let Some(url) = webhook_url {
                        let error_msg = format!(
                            "Command exited with status: {}\nStderr: {}",
                            output.status,
                            stderr.trim()
                        );
                        TaskScheduler::send_alert(url, name, &error_msg).await;
                    }
                }
            }
            Err(e) => {
                error!(
                    "[{}] -> Execution error: Failed to run command '{}': {}",
                    name, command, e
                );
            }
        }
    }

    async fn send_alert(webhook_url: &str, task_name: &str, message: &str) {
        let client = Client::new();
        let payload = json!({
            "text": format!("**Chronsync Task Failed** \n\n**Task:** `{}`\n**Error** {}", task_name, message)
        });

        match client.post(webhook_url).json(&payload).send().await {
            Ok(res) => {
                if res.status().is_success() {
                    info!("[{}] Webhook alert sent successfully.", task_name);
                } else {
                    error!(
                        "[{}] Failed to send webhook. Status: {}",
                        task_name,
                        res.status()
                    );
                }
            }
            Err(e) => {
                error!("[{}] Failed to send webhook: {}", task_name, e);
            }
        }
    }
}
