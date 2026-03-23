use clap::Parser;
use std::env;
use std::path::{Path, PathBuf};
use std::process::{Command, exit};

mod layout;
use layout::LayoutConfig;

#[derive(Parser)]
#[command(name = "tsps")]
#[command(about = "Quickly set up tmux workspaces by splitting windows into multiple panes")]
#[command(version)]
struct Cli {
    /// Layout file to use (YAML format)
    #[arg(short, long, value_name = "LAYOUT_FILE")]
    layout: Option<PathBuf>,

    /// Directory to use (overrides layout file directory)
    #[arg(short, long, value_name = "DIRECTORY")]
    directory: Option<PathBuf>,

    /// Delay before executing commands in milliseconds
    #[arg(short = 'D', long, value_name = "MS", default_value = "2000")]
    delay: u64,

    /// Remaining arguments for traditional usage (pane_count and directory)
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    args: Vec<String>,
}

fn main() {
    let cli = Cli::parse();

    // Check if we're in a tmux session
    if env::var("TMUX").is_err() {
        eprintln!("Error: Not in a tmux session");
        exit(1);
    }

    // If layout file is specified
    if let Some(layout_file) = cli.layout {
        match apply_layout_file(&layout_file, cli.directory.as_ref(), cli.delay) {
            Ok(()) => return,
            Err(e) => {
                eprintln!("Error: Failed to apply layout: {}", e);
                exit(1);
            }
        }
    }

    // Traditional argument-based execution
    if cli.args.len() != 2 {
        eprintln!("Error: Either specify --layout or provide PANE_COUNT and DIRECTORY");
        eprintln!("Usage: tsps <PANE_COUNT> <DIRECTORY>");
        eprintln!("   or: tsps --layout <LAYOUT_FILE> [--directory <DIRECTORY>]");
        exit(1);
    }

    let pane_count = match cli.args[0].parse::<u32>() {
        Ok(count) => count,
        Err(_) => {
            eprintln!(
                "Error: PANE_COUNT must be a positive integer, got: {}",
                cli.args[0]
            );
            exit(1);
        }
    };

    let directory = &cli.args[1];

    if pane_count < 1 {
        eprintln!("Error: pane_count must be a positive integer");
        exit(1);
    }

    let target_dir = Path::new(&directory);
    if !target_dir.exists() {
        eprintln!("Error: Directory '{}' does not exist", directory);
        exit(1);
    }

    // Get absolute path
    let absolute_path = match target_dir.canonicalize() {
        Ok(path) => path,
        Err(e) => {
            eprintln!("Error: Cannot resolve path '{}': {}", directory, e);
            exit(1);
        }
    };

    let path_str = absolute_path.to_string_lossy();

    // Move current pane to target directory
    if let Err(e) = Command::new("tmux")
        .args(["send-keys", &format!("cd '{}'", path_str), "Enter"])
        .status()
    {
        eprintln!("Error: Failed to execute tmux command: {}", e);
        exit(1);
    }

    // Create additional panes (pane_count - 1)
    for i in 1..pane_count {
        let split_direction = if i % 2 == 1 { "-h" } else { "-v" };

        if let Err(e) = Command::new("tmux")
            .args(["split-window", split_direction, "-c", &path_str])
            .status()
        {
            eprintln!("Error: Failed to create pane {}: {}", i + 1, e);
            exit(1);
        }
    }

    // Arrange panes in a tiled layout
    if let Err(e) = Command::new("tmux")
        .args(["select-layout", "tiled"])
        .status()
    {
        eprintln!("Error: Failed to arrange panes: {}", e);
        exit(1);
    }

    println!("Created {} panes in directory: {}", pane_count, path_str);
}

/// Apply layout file
fn apply_layout_file(
    layout_file: &PathBuf,
    override_directory: Option<&PathBuf>,
    delay: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut layout = LayoutConfig::from_file(layout_file)?;

    // Override directory if specified
    if let Some(dir) = override_directory {
        layout.workspace.directory = dir.to_string_lossy().to_string();
    }

    layout.apply_to_tmux(delay)?;
    Ok(())
}
