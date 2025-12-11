mod config;
mod scheduler;
mod utils;
mod watcher;
use log::{debug, LevelFilter};
use simple_logger::SimpleLogger;
mod cli;
mod commands;
use clap::Parser;
use cli::{Cli, Commands};
use commands::{
    handle_check_command, handle_edit_command, handle_init_command, handle_list_command,
    handle_run_command,
};

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let log_level = if cli.verbose {
        LevelFilter::Debug
    } else {
        LevelFilter::Info
    };

    SimpleLogger::new()
        .with_level(log_level)
        .init()
        .expect("Failed to initialize logger");

    debug!("Parsed CLI: {:?}", cli);

    match cli.command {
        Commands::Run(args) => {
            handle_run_command(args).await;
        }
        Commands::List(args) => {
            debug!("Dispatching to handle_list_command");
            handle_list_command(args);
        }
        Commands::Init(args) => {
            handle_init_command(args);
        }
        Commands::Edit(args) => {
            handle_edit_command(args);
        }
        Commands::Check(args) => {
            handle_check_command(args);
        }
    }
}

