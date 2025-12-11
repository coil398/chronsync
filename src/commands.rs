use crate::cli::{CheckArgs, EditArgs, ExecArgs, InitArgs, ListArgs, RunArgs};
use crate::cli::{ServiceAction, ServiceArgs};
use crate::config;
use crate::config::load_config;
use crate::scheduler::TaskScheduler;
use crate::utils;
use crate::watcher;
use log::{debug, error, info};
use service_manager::{
    RestartPolicy, ServiceInstallCtx, ServiceLabel, ServiceLevel, ServiceManager, ServiceStartCtx,
    ServiceStopCtx, ServiceUninstallCtx,
};
use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process;
use std::process::Command;
use tokio::sync::mpsc;
use utils::get_config_path;

pub async fn handle_run_command(args: RunArgs) {
    debug!("Entered handle_run_command with args: {:?}", args);
    let config_path = match args.config_path {
        Some(p) => p,
        None => match get_config_path() {
            Ok(p) => p,
            Err(e) => {
                error!("Initialization Error: {}", e);
                process::exit(1);
            }
        },
    };

    debug!("Resolved config path: {}", config_path.display());

    if !config_path.exists() {
        error!("Error: Configuration file not found at path:");
        error!("-> Path: {}", config_path.display());
        process::exit(1);
    }

    match core_check_config(&config_path) {
        Ok(_) => {
            info!("Configuration validated successfully.");
        }
        Err(e) => {
            error!("Configuration check failed. Cannot start daemon.");
            eprintln!("\n{}\n", e);
            process::exit(1);
        }
    }

    let (tx_reload, mut rx_reload) = mpsc::channel::<()>(1);

    let mut scheduler = TaskScheduler::new();

    let watcher_path = config_path.clone();
    let tx_clone = tx_reload.clone();

    tokio::spawn(async move {
        if let Err(e) = watcher::start_watcher(&watcher_path, tx_clone).await {
            error!("Watcher failed: {:?}", e);
        }
    });

    info!("chronsync Daemon started.");

    match load_config(&config_path) {
        Ok(c) => {
            info!("Configuration loaded. {} tasks.", c.tasks.len());
            scheduler.reload_tasks(c);
        }
        Err(e) => {
            error!("[Main] Failed to load initial config. Existing: {}", e);
            return;
        }
    };

    loop {
        tokio::select! {
            Some(_) = rx_reload.recv() => {
                info!("\n>>> CONFIG CHANGE DETECTED! RELOADING... <<<");

                match load_config(&config_path) {
                    Ok(new_config) => {
                        scheduler.reload_tasks(new_config);
                        info!("New configuration applied. Tasks reloaded.");
                    },
                    Err(e) => {
                        error!("Error reloading configuration (Configuration rejected): {}", e);
                    }
                }
            }
            _ = tokio::signal::ctrl_c() => {
                info!("\nCtrl+C received. Shutting down gracefully...");
                scheduler.reload_tasks(config::Config { tasks: vec![] });
                break;
            }
        }
    }
}

pub fn handle_list_command(args: ListArgs) {
    debug!("Entered handle_list_command with args: {:?}", args);
    let config_path = match args.config_path {
        Some(p) => p,
        None => match get_config_path() {
            Ok(p) => p,
            Err(e) => {
                error!("Error: Failed to determine configuration path.");
                error!("Reason: {}", e);
                process::exit(1);
            }
        },
    };

    debug!("Resolved config path: {}", config_path.display());

    if !config_path.exists() {
        error!("Error: Configuration file not found at path:");
        error!("-> Path: {}", config_path.display());
        process::exit(1);
    }

    match load_config(&config_path) {
        Ok(config) => {
            println!("Configuration loaded from: {}", config_path.display());
            println!(
                "\n--- chronsync Task List ({} Tasks) ---",
                config.tasks.len()
            );
            for task in config.tasks {
                println!("- [{}]: {}\n", task.name, task.cron_schedule.to_string());
                println!(
                    "  Command: {} {:?}",
                    task.command,
                    task.args.unwrap_or_default()
                );
                println!("-----------------------------");
            }
        }
        Err(e) => {
            error!("Error loading configuration: {}", e);
            error!("The configuration file contains invalid JSON or an invalid cron schedule.");
            process::exit(1);
        }
    }
}

pub fn handle_init_command(args: InitArgs) {
    debug!("Entered handle_init_command with args: {:?}", args);
    let config_path = match args.config_path {
        Some(p) => p,
        None => match get_config_path() {
            Ok(p) => p,
            Err(e) => {
                error!("Initialization Error: {}", e);
                process::exit(1);
            }
        },
    };

    debug!("Resolved config path: {}", config_path.display());

    let parent_dir = config_path.parent().unwrap_or_else(|| {
        error!("Could not determine parent directory for config file.");
        process::exit(1);
    });
    fs::create_dir_all(parent_dir).unwrap_or_else(|e| {
        error!("Failed to create directory {}: {}", parent_dir.display(), e);
        process::exit(1);
    });

    if config_path.exists() {
        println!(
            "Configuration file already exists at: {}",
            config_path.display()
        );
        print!("Do you want to overwrite it? (y/N): ");
        io::stdout().flush().unwrap();

        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();

        let should_overwrite = input.trim().to_lowercase() == "y";

        if !should_overwrite {
            info!("Initialization cancelled by user.");
            return;
        }
    }

    let initial_config_content = r#"{{
        "tasks": [
        {{
            "name": "sample_ping",
            "cron_schedule": "*/10 * * * * *",
            "command": "/bin/sh",
            "args": [
                "-c", "/bin/echo \"[Sample] Check at $(date)\""
            ]
        }},
        {{
            "name": "sample_cleanup",
            "cron_schedule": "0 0 0 * * *",
            "command": "usr/bin/find",
            "args": ["/tmp", "-type", "f", "-atime", "+7", "-delete"]
        }}
        ]
    }}"#;

    fs::write(&config_path, initial_config_content).unwrap_or_else(|e| {
        error!(
            "Failed to write configuration file to {}: {}",
            config_path.display(),
            e
        );
        process::exit(1);
    });

    println!("\nSuccessfully created initial configuration file.");
    println!("  Path: {}", config_path.display());
    println!("\nNext steps:");
    println!("1. Edit the file to define your tasks.");
    println!("2. Run the daemon: `chronosync run`");
}

pub fn core_check_config(config_path: &PathBuf) -> Result<(), String> {
    if !config_path.exists() {
        return Err(format!(
            "Configuration file not found at: {}",
            config_path.display()
        ));
    }

    match load_config(config_path) {
        Ok(config) => {
            info!(
                "Configuration check successful: {} tasks loaded.",
                config.tasks.len()
            );
            Ok(())
        }
        Err(e) => Err(format!(
            "Validation failed: Invalid JSON or Cron Schedule.\n  Details: {}",
            e
        )),
    }
}

pub fn handle_edit_command(args: EditArgs) {
    debug!("Entered handle_edit_command with args: {:?}", args);
    let config_path = match args.config_path {
        Some(p) => p,
        None => match get_config_path() {
            Ok(p) => p,
            Err(e) => {
                error!("Error: Failed to determine configuration path.");
                error!("Reason: {}", e);
                process::exit(1);
            }
        },
    };

    debug!("Resolved config path: {}", config_path.display());

    if !config_path.exists() {
        error!("Error: Configuration file not found at path:");
        error!("-> Path: {}", config_path.display());
        process::exit(1);
    }

    let editor = env::var("EDITOR")
        .or_else(|_| env::var("VISUAL"))
        .unwrap_or_else(|_| {
            error!("$EDITOR or $VISUAL environment variable not set. Falling back to 'vi'.");
            "vi".to_string()
        });

    info!("Opening config file with editor: {}", editor);

    let status = Command::new(&editor)
        .arg(&config_path)
        .status()
        .unwrap_or_else(|e| {
            error!("Failed to execute editor '{}': {}", editor, e);
            process::exit(1);
        });

    if !status.success() {
        error!("Editor process exited with an error status: {}", status);
        process::exit(1);
    }

    info!("Configuration file edited. The daemon will reload automatically.");

    match core_check_config(&config_path) {
        Ok(_) => {
            info!("Configuration saved and validated successfully.");
            info!("The daemon will reload automatically");
        }
        Err(e) => {
            error!("\n Validation failed after editing! The daemon WILL NOT reload this file.");
            eprintln!("{}\n", e)
        }
    }
}

pub fn handle_check_command(args: CheckArgs) {
    debug!("Entered handle_check_command with args: {:?}", args);
    let config_path = match args.config_path {
        Some(p) => p,
        None => match get_config_path() {
            Ok(p) => p,
            Err(e) => {
                error!("Error: Failed to determine configuration path.");
                error!("Reason: {}", e);
                process::exit(1);
            }
        },
    };

    debug!("Resolved config path: {}", config_path.display());

    if !config_path.exists() {
        error!("Error: Configuration file not found at path:");
        error!("-> Path: {}", config_path.display());
        process::exit(1);
    }

    match core_check_config(&config_path) {
        Ok(_) => {
            println!("Configuration check passed.");
        }
        Err(e) => {
            error!("{}", e);
            process::exit(1);
        }
    }
}

pub fn handle_service_command(args: ServiceArgs, user: bool) {
    let label: ServiceLabel = "chronsync".parse().unwrap();

    let mut manager = <dyn ServiceManager>::native().expect("Failed to detect service manager");

    if user {
        manager
            .set_level(ServiceLevel::User)
            .expect("Failed to set service level to User");
    }

    match args.action {
        ServiceAction::Install => {
            let exe_path = env::current_exe().unwrap_or_else(|_| {
                error!("Failed to get current executable path");
                process::exit(1);
            });

            info!("Installing service for binary: {}", exe_path.display());

            let install_result = manager.install(ServiceInstallCtx {
                label: label.clone(),
                program: exe_path,
                args: vec!["run".into()],
                contents: None,
                username: None,
                working_directory: None,
                environment: None,
                autostart: true,
                restart_policy: RestartPolicy::OnFailure {
                    delay_secs: Some(10),
                },
            });

            match install_result {
                Ok(_) => {
                    info!("Service installed successfully.");
                    println!("To start the service immediately, run: `chronsync service start`");
                }
                Err(e) => {
                    error!("Failed to install service: {}", e);
                }
            }
        }
        ServiceAction::Uninstall => {
            match manager.uninstall(ServiceUninstallCtx {
                label: label.clone(),
            }) {
                Ok(_) => {
                    info!("Service uninstalled.");
                }
                Err(e) => {
                    error!("Failed to uninstall: {}", e);
                }
            }
        }
        ServiceAction::Start => {
            match manager.start(ServiceStartCtx {
                label: label.clone(),
            }) {
                Ok(_) => {
                    info!("Service started.");
                }
                Err(e) => {
                    error!("Failed to start: {}", e);
                }
            }
        }
        ServiceAction::Stop => {
            match manager.stop(ServiceStopCtx {
                label: label.clone(),
            }) {
                Ok(_) => {
                    info!("Service stopped.");
                }
                Err(e) => {
                    error!("Failed to stop: {}", e);
                }
            }
        }
        ServiceAction::Log(log_args) => {
            let mut cmd = Command::new("journalctl");

            if user {
                cmd.arg("--user");
            }

            cmd.arg("-u").arg("chronsync");

            if log_args.follow {
                cmd.arg("-f");
            }

            cmd.arg("-n").arg(log_args.lines.to_string());

            info!("Executing log command: {:?}", cmd);

            let status = cmd.status().unwrap_or_else(|e| {
                error!("Failed to execute journalctl: {}", e);
                process::exit(1);
            });

            if !status.success() {
                // journalctl returns non-zero if no entries found or error
                // We don't need to panic, just log it.
                // However, users might just Ctrl+C, which is fine.
            }
        }
    }
}

pub async fn handle_exec_command(args: ExecArgs) {
    let config_path = match args.config_path {
        Some(p) => p,
        None => match utils::get_config_path() {
            Ok(p) => p,
            Err(e) => {
                error!("Error: Failed to determine configuration path.");
                error!("Reason: {}", e);
                process::exit(1);
            }
        },
    };

    debug!("Resolved config path: {}", config_path.display());

    if !config_path.exists() {
        error!("Error: Configuration file not found at path:");
        error!("-> Path: {}", config_path.display());
        process::exit(1);
    }

    match core_check_config(&config_path) {
        Ok(_) => {
            info!("Configuration validated successfully.");
        }
        Err(e) => {
            error!("Configuration check failed. Cannot start daemon.");
            eprintln!("\n{}\n", e);
            process::exit(1);
        }
    }

    let config = match load_config(&config_path) {
        Ok(c) => c,
        Err(e) => {
            error!("Failed to load config: {}", e);
            return;
        }
    };

    let target_task = config.tasks.iter().find(|t| t.name == args.task_name);

    match target_task {
        Some(task) => {
            info!("Manually executing task: '{}'", task.name);

            TaskScheduler::execute_command(
                &task.name,
                &task.command,
                &task.args.as_deref().unwrap_or(&[]),
                task.timeout,
                task.webhook_url.as_deref(),
                task.cwd.as_deref(),
                task.env.as_ref(),
            )
            .await;

            info!("Manual execution finished.");
        }
        None => {
            error!("Task '{}' not found in configuration.", args.task_name);
            let available: Vec<&String> = config.tasks.iter().map(|t| &t.name).collect();
            error!("Available tasks: {:?}", available);
            process::exit(1);
        }
    }
}
