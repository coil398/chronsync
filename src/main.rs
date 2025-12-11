use config::load_config;
use std::process;
mod config;
mod watcher;
use tokio::sync::mpsc;
mod scheduler;
use directories::UserDirs;
use scheduler::TaskScheduler;
use std::path::PathBuf;

fn get_config_path() -> Result<PathBuf, String> {
    if let Some(user_dirs) = UserDirs::new() {
        let home_dir = user_dirs.home_dir();
        let config_path = home_dir.join(".config").join("chronosync").join("config.json");

        return Ok(config_path);
    }

    Err("Could not determine user home directory.".to_string())
}

#[tokio::main]
async fn main() {
    let config_path = match get_config_path() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Initialization Error: {}", e);
            process::exit(1);
        }
    };

    if !config_path.exists() {
        eprintln!("Initialization Error: Configuration file not found at path:");
        eprintln!("-> Path: {}", config_path.display());
        process::exit(1);
    }

    let (tx_reload, mut rx_reload) = mpsc::channel::<()>(1);

    let mut scheduler = TaskScheduler::new();

    let watcher_path = config_path.clone();
    let tx_clone = tx_reload.clone();

    tokio::spawn(async move {
        if let Err(e) = watcher::start_watcher(&watcher_path, tx_clone).await {
            eprintln!("Watcher failed: {:?}", e);
        }
    });

    println!("[Main] Chronosync Daemon started.");

    match load_config(&config_path) {
        Ok(c) => {
            println!("[Main] Initial config loaded. {} tasks.", c.tasks.len());
            scheduler.reload_tasks(c);
        }
        Err(e) => {
            eprintln!("[Main] Failed to load initial config. Existing: {}", e);
            return;
        }
    };

    loop {
        tokio::select! {
            Some(_) = rx_reload.recv() => {
                println!("\n>>> CONFIG CHANGE DETECTED! RELOADING... <<<");

                match load_config(&config_path) {
                    Ok(new_config) => {
                        scheduler.reload_tasks(new_config);
                        println!("[Main] New configuration applied. Tasks reloaded.");
                    },
                    Err(e) => {
                        eprintln!("[Main] Error reloading configuration (Configuration rejected): {}", e);
                    }
                }
            }
            _ = tokio::signal::ctrl_c() => {
                println!("\n[Main] Ctrl+C received. Shutting down gracefully...");
                scheduler.reload_tasks(config::Config { tasks: vec![] });
                break;
            }
        }
    }
}
