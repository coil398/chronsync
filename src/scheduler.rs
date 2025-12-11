use crate::config::{Config, Task};
use cron::Schedule;
use std::sync::{Arc, Mutex};
use tokio::process::Command;
use tokio::task::JoinHandle;
use tokio::time::sleep;

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
        println!(
            "[Scheduler] Stopping {} existing tasks...",
            self.job_handles.len()
        );

        for handle_mutex in self.job_handles.drain(..) {
            if let Some(handle) = handle_mutex.lock().unwrap().take() {
                handle.abort();
            }
        }

        println!(
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

        println!(
            "[Scheduler] Registering task '{}' with schedule: {}",
            name, schedule
        );

        let job_task = tokio::spawn(async move {
            TaskScheduler::run_job_loop(name, schedule, task.command, task.args).await;

            handle_ref_for_job.lock().unwrap().take();
        });

        *handle_ref.lock().unwrap() = Some(job_task);
    }

    async fn run_job_loop(
        name: String,
        schedule: Schedule,
        command: String,
        args: Option<Vec<String>>,
    ) {
        let mut job_running = true;

        while job_running {
            let now = chrono::Local::now();

            if let Some(next_execution) = schedule.upcoming(chrono::Local).next() {
                let delay = next_execution - now;
                let duration = delay.to_std().unwrap_or_default();

                sleep(duration).await;

                TaskScheduler::execute_command(&name, &command, args.as_deref().unwrap_or(&[]))
                    .await;
            } else {
                eprintln!(
                    "[{}] Schedule ended or failed to calculate next time.",
                    name
                );
                job_running = false;
            }
        }
    }

    async fn execute_command(name: &str, command: &str, args: &[String]) {
        println!("[{}] -> Command starting: {} {:?}", name, command, args);

        let mut cmd_to_run = Command::new(command);
        cmd_to_run.args(args);

        match cmd_to_run.output().await {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);

                if output.status.success() {
                    println!("[{}] -> Command SUCCESS. Status: {}", name, output.status);
                    if !stdout.trim().is_empty() {
                        println!("[{}] -> STDOUT:\n{}", name, stdout.trim());
                    }
                } else {
                    eprintln!("[{}] -> Command FAILED. Status: {}", name, output.status);
                    if !stderr.trim().is_empty() {
                        eprintln!("[{}] -> STDERR:\n{}", name, stderr.trim());
                    }
                }
            }
            Err(e) => {
                eprintln!(
                    "[{}] -> Execution error: Failed to run command '{}': {}",
                    name, command, e
                );
            }
        }
    }
}
