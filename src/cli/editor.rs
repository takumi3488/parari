use std::env;
use std::fs;
use std::io::Write;
use std::process::Command;

use crate::error::{Error, Result};
use tempfile::NamedTempFile;

/// Opens an editor for the user to enter a prompt.
/// Uses $EDITOR environment variable, falling back to vi.
/// Returns the entered text, or an error if the editor fails or returns empty input.
pub fn open_editor_for_prompt() -> Result<String> {
    let editor = env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());

    // Create a temporary file with instructions
    let mut temp_file = NamedTempFile::new().map_err(|e| Error::EditorFailed {
        message: format!("Failed to create temporary file: {}", e),
    })?;

    // Write initial content with instructions
    let initial_content = "\n# Enter your prompt above this line.\n# Lines starting with '#' will be ignored.\n# Save and exit the editor to continue.\n# Leave empty to cancel.\n";
    temp_file
        .write_all(initial_content.as_bytes())
        .map_err(|e| Error::EditorFailed {
            message: format!("Failed to write to temporary file: {}", e),
        })?;

    let temp_path = temp_file.path().to_path_buf();

    // Open the editor
    let status = Command::new(&editor)
        .arg(&temp_path)
        .status()
        .map_err(|e| Error::EditorFailed {
            message: format!("Failed to start editor '{}': {}", editor, e),
        })?;

    if !status.success() {
        return Err(Error::EditorFailed {
            message: format!("Editor '{}' exited with non-zero status", editor),
        });
    }

    // Read the result
    let content = fs::read_to_string(&temp_path).map_err(|e| Error::EditorFailed {
        message: format!("Failed to read temporary file: {}", e),
    })?;

    // Filter out comment lines and trim
    let prompt: String = content
        .lines()
        .filter(|line| !line.starts_with('#'))
        .collect::<Vec<&str>>()
        .join("\n")
        .trim()
        .to_string();

    if prompt.is_empty() {
        return Err(Error::EditorFailed {
            message: "No prompt entered".to_string(),
        });
    }

    Ok(prompt)
}

#[cfg(test)]
mod tests {

    #[test]
    fn test_filter_comments() {
        let content = "Hello World\n# This is a comment\nSecond line\n# Another comment";
        let filtered: String = content
            .lines()
            .filter(|line| !line.starts_with('#'))
            .collect::<Vec<&str>>()
            .join("\n")
            .trim()
            .to_string();
        assert_eq!(filtered, "Hello World\nSecond line");
    }

    #[test]
    fn test_empty_after_filter() {
        let content = "# Comment only\n# Another comment\n";
        let filtered: String = content
            .lines()
            .filter(|line| !line.starts_with('#'))
            .collect::<Vec<&str>>()
            .join("\n")
            .trim()
            .to_string();
        assert!(filtered.is_empty());
    }
}
