use crate::cli::{CheckArgs, EditArgs, InitArgs, ListArgs, RunArgs};
use crate::config;
use crate::config::load_config;
use crate::scheduler::TaskScheduler;
use crate::utils;
use crate::watcher;
use log::{debug, error, info};
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
        error!("Initialization Error: Configuration file not found at path:");
        error!("-> Path: {}", config_path.display());
        process::exit(1);
    }

    match core_check_config(&config_path) {
        Ok(_) => {
            info!("Initial configuration validated successfully.");
        }
        Err(e) => {
            error!("Initial configuration check failed. Cannot start daemon.");
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

    info!("[Main] chronsync Daemon started.");

    match load_config(&config_path) {
        Ok(c) => {
            info!("[Main] Initial config loaded. {} tasks.", c.tasks.len());
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
                        info!("[Main] New configuration applied. Tasks reloaded.");
                    },
                    Err(e) => {
                        error!("[Main] Error reloading configuration (Configuration rejected): {}", e);
                    }
                }
            }
            _ = tokio::signal::ctrl_c() => {
                info!("\n[Main] Ctrl+C received. Shutting down gracefully...");
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
