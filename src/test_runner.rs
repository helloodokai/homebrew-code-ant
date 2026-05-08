use anyhow::{Result, Context};
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct TestRunner {
    cmd: String,
    repo_root: PathBuf,
}

impl TestRunner {
    pub fn new(cmd: String, repo_root: PathBuf) -> Self {
        TestRunner { cmd, repo_root }
    }

    pub fn detect(repo_root: &Path, override_cmd: Option<String>) -> Result<Option<Self>> {
        if let Some(cmd) = override_cmd {
            return Ok(Some(TestRunner::new(cmd, repo_root.to_path_buf())));
        }
        if let Some(cmd) = auto_detect(repo_root)? {
            return Ok(Some(TestRunner::new(cmd, repo_root.to_path_buf())));
        }
        Ok(None)
    }

    pub fn run(&self) -> Result<bool> {
        let output = if cfg!(target_os = "windows") {
            Command::new("cmd")
                .args(["/C", &self.cmd])
                .current_dir(&self.repo_root)
                .output()
                .with_context(|| "Failed to run test command")?
        } else {
            Command::new("sh")
                .args(["-c", &self.cmd])
                .current_dir(&self.repo_root)
                .output()
                .with_context(|| "Failed to run test command")?
        };
        Ok(output.status.success())
    }
}

fn auto_detect(repo_root: &Path) -> Result<Option<String>> {
    if repo_root.join("pytest.ini").is_file()
        || repo_root.join("setup.cfg").is_file()
        || repo_root.join("pyproject.toml").is_file()
    {
        if has_pytest_tests(repo_root) {
            return Ok(Some("pytest".to_string()));
        }
    }
    if repo_root.join("tests").is_dir() {
        let entries = std::fs::read_dir(repo_root.join("tests"))?;
        for entry in entries {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().to_string();
            if name.ends_with(".py") {
                return Ok(Some("pytest".to_string()));
            }
        }
    }

    if repo_root.join("package.json").is_file() {
        let content = std::fs::read_to_string(repo_root.join("package.json"))?;
        if content.contains("\"test\"") {
            return Ok(Some("npm test".to_string()));
        }
    }

    if repo_root.join("go.mod").is_file() {
        return Ok(Some("go test ./...".to_string()));
    }

    if repo_root.join("Cargo.toml").is_file() {
        return Ok(Some("cargo test".to_string()));
    }

    if repo_root.join("pom.xml").is_file() {
        return Ok(Some("mvn test".to_string()));
    }

    Ok(None)
}

fn has_pytest_tests(repo_root: &Path) -> bool {
    let dirs = [
        &repo_root.join("tests"),
        &repo_root.join("test"),
        &repo_root.join("src"),
    ];
    for dir in dirs {
        if dir.is_dir() {
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if name.ends_with(".py") {
                        return true;
                    }
                }
            }
        }
    }
    true
}
