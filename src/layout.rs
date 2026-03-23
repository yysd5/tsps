use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::process::Command;

/// Complete structure of layout configuration file
#[derive(Debug, Deserialize, Serialize)]
pub struct LayoutConfig {
    pub workspace: WorkspaceConfig,
    pub panes: Vec<PaneConfig>,
}

/// Workspace configuration
#[derive(Debug, Deserialize, Serialize)]
pub struct WorkspaceConfig {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    pub directory: String,
}

/// Individual pane configuration
#[derive(Debug, Deserialize, Serialize)]
pub struct PaneConfig {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub size: Option<String>,
    #[serde(default)]
    pub split: Option<String>, // "horizontal" or "vertical"
    #[serde(default)]
    pub commands: Vec<String>,
    #[serde(default)]
    pub focus: bool,
}

impl LayoutConfig {
    /// Load layout configuration from YAML file
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error>> {
        let content = fs::read_to_string(path)?;
        let config: LayoutConfig = serde_yaml::from_str(&content)?;
        Ok(config)
    }

    /// Apply layout to tmux
    pub fn apply_to_tmux(&self, delay: u64) -> Result<(), Box<dyn std::error::Error>> {
        // Check if directory exists
        let target_dir = Path::new(&self.workspace.directory);
        if !target_dir.exists() {
            return Err(format!("Directory '{}' does not exist", self.workspace.directory).into());
        }

        // Get absolute path
        let absolute_path = target_dir.canonicalize()?;
        let path_str = absolute_path.to_string_lossy();

        // Move current pane to target directory
        Command::new("tmux")
            .args(["send-keys", &format!("cd '{}'", path_str), "Enter"])
            .status()?;

        // Create additional panes
        self.create_panes(&path_str)?;

        // Apply layout
        self.arrange_layout()?;

        // Adjust pane sizes
        self.adjust_pane_sizes()?;

        // Execute commands in each pane
        self.execute_commands(delay)?;

        // Set focus
        self.set_focus()?;

        println!(
            "Applied layout '{}' in directory: {}",
            self.workspace.name, path_str
        );
        Ok(())
    }

    /// Create panes
    fn create_panes(&self, base_path: &str) -> Result<(), Box<dyn std::error::Error>> {
        for (index, pane) in self.panes.iter().enumerate().skip(1) {
            let split_direction = match pane.split.as_deref() {
                Some("horizontal") => "-v",
                Some("vertical") => "-h",
                _ => {
                    if index % 2 == 1 {
                        "-h"
                    } else {
                        "-v"
                    }
                } // Default alternating split
            };

            let args = vec!["split-window", split_direction, "-c", base_path];

            Command::new("tmux")
                .args(&args)
                .status()
                .map_err(|e| format!("Failed to create pane {}: {}", index + 1, e))?;
        }
        Ok(())
    }

    /// Arrange layout
    fn arrange_layout(&self) -> Result<(), Box<dyn std::error::Error>> {
        // Always apply tiled layout first to create 2x2 grid
        Command::new("tmux")
            .args(["select-layout", "tiled"])
            .status()
            .map_err(|e| format!("Failed to arrange panes: {}", e))?;
        Ok(())
    }

    /// Adjust pane sizes after layout is applied
    fn adjust_pane_sizes(&self) -> Result<(), Box<dyn std::error::Error>> {
        // Wait for layout to settle
        std::thread::sleep(std::time::Duration::from_millis(500));

        // First pass: adjust individual pane sizes
        for (index, pane) in self.panes.iter().enumerate() {
            if let Some(size) = &pane.size {
                let split_direction = pane.split.as_deref().unwrap_or("vertical");

                if size.ends_with('%') {
                    let percentage = size.trim_end_matches('%');

                    // Resize based on split direction
                    let resize_direction = match split_direction {
                        "horizontal" => "-y", // Height for horizontal splits
                        "vertical" => "-x",   // Width for vertical splits
                        _ => "-x",
                    };

                    Command::new("tmux")
                        .args([
                            "resize-pane",
                            "-t",
                            &index.to_string(),
                            resize_direction,
                            &format!("{}%", percentage),
                        ])
                        .status()
                        .map_err(|e| {
                            format!(
                                "Failed to resize pane {} {}: {}",
                                index, resize_direction, e
                            )
                        })?;
                } else {
                    // Fixed size (lines or columns)
                    let size_value = size.parse::<i32>().unwrap_or(10);
                    let resize_direction = match split_direction {
                        "horizontal" => "-y", // Height for horizontal splits
                        "vertical" => "-x",   // Width for vertical splits
                        _ => "-y",
                    };

                    Command::new("tmux")
                        .args([
                            "resize-pane",
                            "-t",
                            &index.to_string(),
                            resize_direction,
                            &size_value.to_string(),
                        ])
                        .status()
                        .map_err(|e| {
                            format!(
                                "Failed to resize pane {} {}: {}",
                                index, resize_direction, e
                            )
                        })?;
                }
            }
        }

        // Second pass: adjust row heights by resizing the first horizontal split
        // Find the first pane with horizontal split (usually the terminal pane)
        for (index, pane) in self.panes.iter().enumerate() {
            if pane.split.as_deref() == Some("horizontal") {
                if let Some(size) = pane.size.as_ref().filter(|s| s.ends_with('%')) {
                    let percentage = size.trim_end_matches('%');
                    // This controls the overall top/bottom ratio
                    Command::new("tmux")
                        .args([
                            "resize-pane",
                            "-t",
                            &index.to_string(),
                            "-y",
                            &format!("{}%", percentage),
                        ])
                        .status()
                        .map_err(|e| {
                            format!("Failed to adjust row height for pane {}: {}", index, e)
                        })?;
                }
                break; // Only adjust the first horizontal split
            }
        }

        Ok(())
    }

    /// Execute commands in each pane
    fn execute_commands(&self, delay: u64) -> Result<(), Box<dyn std::error::Error>> {
        // Wait for all panes to settle after creation and resizing
        std::thread::sleep(std::time::Duration::from_millis(delay));

        for (index, pane) in self.panes.iter().enumerate() {
            if pane.commands.is_empty() {
                continue;
            }

            // Select pane
            Command::new("tmux")
                .args(["select-pane", "-t", &index.to_string()])
                .status()?;

            // Delay between pane selection and command execution
            std::thread::sleep(std::time::Duration::from_millis(300));

            // Execute commands
            for command in &pane.commands {
                Command::new("tmux")
                    .args(["send-keys", command, "Enter"])
                    .status()
                    .map_err(|e| {
                        format!(
                            "Failed to execute command '{}' in pane {}: {}",
                            command, index, e
                        )
                    })?;
            }
        }
        Ok(())
    }

    /// Set focus
    fn set_focus(&self) -> Result<(), Box<dyn std::error::Error>> {
        for (index, pane) in self.panes.iter().enumerate() {
            if pane.focus {
                Command::new("tmux")
                    .args(["select-pane", "-t", &index.to_string()])
                    .status()?;
                break;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_yaml_parsing() {
        let yaml_content = r#"
workspace:
  name: "test-layout"
  description: "Test layout for development"
  directory: "/tmp"

panes:
  - id: "editor"
    commands:
      - "echo 'Editor pane'"
    focus: true
  - id: "terminal"
    split: "horizontal"
    commands:
      - "echo 'Terminal pane'"
"#;

        let config: LayoutConfig = serde_yaml::from_str(yaml_content).unwrap();
        assert_eq!(config.workspace.name, "test-layout");
        assert_eq!(config.panes.len(), 2);
        assert!(config.panes[0].focus);
    }

    #[test]
    fn test_yaml_parsing_with_size() {
        let yaml_content = r#"
workspace:
  name: "test-layout-with-size"
  description: "Test layout with size specification"
  directory: "/tmp"

panes:
  - id: "main"
    commands:
      - "echo 'Main pane'"
    focus: true
  - id: "sidebar"
    split: "vertical"
    size: "30%"
    commands:
      - "echo 'Sidebar pane'"
  - id: "bottom"
    split: "horizontal"
    size: "10"
    commands:
      - "echo 'Bottom pane'"
"#;

        let config: LayoutConfig = serde_yaml::from_str(yaml_content).unwrap();
        assert_eq!(config.workspace.name, "test-layout-with-size");
        assert_eq!(config.panes.len(), 3);
        assert!(config.panes[0].focus);
        assert_eq!(config.panes[1].size, Some("30%".to_string()));
        assert_eq!(config.panes[2].size, Some("10".to_string()));
    }
}
