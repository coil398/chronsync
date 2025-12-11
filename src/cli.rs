use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about=None)]
pub struct Cli {
    #[arg(short, long, global = true)]
    pub verbose: bool,

    #[arg(short = 't', long, global = true)]
    pub worker_threads: Option<usize>,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    Run(RunArgs),
    List(ListArgs),
    Init(InitArgs),
    Edit(EditArgs),
    Check(CheckArgs),
    Service(ServiceArgs),
    Exec(ExecArgs),
}

#[derive(clap::Args, Debug)]
pub struct RunArgs {
    #[arg(short, long)]
    pub config_path: Option<PathBuf>,
}

#[derive(clap::Args, Debug)]
pub struct ListArgs {
    #[arg(short, long)]
    pub config_path: Option<PathBuf>,
}

#[derive(clap::Args, Debug)]
pub struct InitArgs {
    #[arg(short, long)]
    pub config_path: Option<PathBuf>,
}

#[derive(clap::Args, Debug)]
pub struct EditArgs {
    #[arg(short, long)]
    pub config_path: Option<PathBuf>,
}

#[derive(clap::Args, Debug)]
pub struct CheckArgs {
    #[arg(short, long)]
    pub config_path: Option<PathBuf>,
}

#[derive(clap::Args, Debug)]
pub struct ServiceArgs {
    #[command(subcommand)]
    pub action: ServiceAction,
}

#[derive(Subcommand, Debug)]
pub enum ServiceAction {
    Install,
    Uninstall,
    Start,
    Stop,
}

#[derive(clap::Args, Debug)]
pub struct ExecArgs {
    pub task_name: String,

    #[arg(short, long)]
    pub config_path: Option<PathBuf>,
}
