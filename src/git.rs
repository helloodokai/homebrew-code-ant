use anyhow::{Result, bail};
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct Git {
    repo_root: PathBuf,
    expected_head: std::sync::Mutex<String>,
}

impl Git {
    pub fn new() -> Result<Self> {
        let repo_root = find_repo_root()?;
        let initial_head = run_git(
            &repo_root, &["rev-parse", "HEAD"],
        )?.trim().to_string();
        Ok(Git { repo_root, expected_head: std::sync::Mutex::new(initial_head) })
    }

    #[allow(dead_code)]
    pub fn from_dir(repo_root: PathBuf) -> Result<Self> {
        let initial_head = run_git(
            &repo_root, &["rev-parse", "HEAD"],
        )?.trim().to_string();
        Ok(Git { repo_root, expected_head: std::sync::Mutex::new(initial_head) })
    }

    pub fn repo_root(&self) -> &Path {
        &self.repo_root
    }

    pub fn current_branch(&self) -> Result<String> {
        let branch = run_git(
            &self.repo_root, &["rev-parse", "--abbrev-ref", "HEAD"],
        )?;
        Ok(branch.trim().to_string())
    }

    pub fn is_dirty(&self) -> Result<bool> {
        let status = run_git(
            &self.repo_root, &["status", "--porcelain", "--untracked-files=no"],
        )?;
        Ok(!status.trim().is_empty())
    }

    pub fn create_and_switch_branch(
        &self,
        timestamp: &str,
    ) -> Result<String> {
        let mut candidate = format!("code-ant/{}", timestamp);
        let mut counter = 2u32;
        loop {
            let exists = branch_exists(&self.repo_root, &candidate)?;
            if !exists {
                break;
            }
            candidate = format!("code-ant/{}-{}", timestamp, counter);
            counter += 1;
        }
        run_git(
            &self.repo_root, &["checkout", "-b", &candidate],
        )?;
        Ok(candidate)
    }

    pub fn stage_and_commit(&self, file: &Path, message: &str) -> Result<()> {
        let rel = file.strip_prefix(&self.repo_root).unwrap_or(file);
        run_git(
            &self.repo_root, &["add", &rel.to_string_lossy()],
        )?;
        run_git(
            &self.repo_root, &["commit", "-m", message],
        )?;
        Ok(())
    }

    pub fn reset_hard_to_head(&self) -> Result<()> {
        run_git(
            &self.repo_root, &["reset", "--hard", "HEAD"],
        )?;
        Ok(())
    }

    pub fn checkout_file(&self, file: &Path) -> Result<()> {
        let rel = file.strip_prefix(&self.repo_root).unwrap_or(file);
        run_git(
            &self.repo_root, &["checkout", "--", &rel.to_string_lossy()],
        )?;
        Ok(())
    }

    pub fn verify_clean(&self) -> Result<()> {
        if self.is_dirty()? {
            bail!(crate::error::CodeAntError::DirtyWorkingTree);
        }
        let current_head = run_git(
            &self.repo_root, &["rev-parse", "HEAD"],
        )?;
        let expected = self.initial_head();
        if current_head.trim() != expected {
            bail!(crate::error::CodeAntError::DivergentHead);
        }
        Ok(())
    }

    pub fn initial_head(&self) -> String {
        let lock = self.expected_head.lock().unwrap();
        lock.clone()
    }

    pub fn update_expected_head(&self) -> Result<()> {
        let current = run_git(
            &self.repo_root, &["rev-parse", "HEAD"])?.trim().to_string();
        let mut lock = self.expected_head.lock().unwrap();
        *lock = current;
        Ok(())
    }
}

fn find_repo_root() -> Result<PathBuf> {
    let mut current = std::env::current_dir()?;
    loop {
        if current.join(".git").is_dir() {
            return Ok(current.canonicalize()?);
        }
        if !current.pop() {
            bail!(crate::error::CodeAntError::NotGitRepo);
        }
    }
}

fn run_git(
    repo_root: &Path,
    args: &[&str],
) -> Result<String> {
    let output = Command::new("git")
        .current_dir(repo_root)
        .args(args)
        .output()
        .map_err(|e| anyhow::anyhow!("Failed to run git {:?}: {}", args, e))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("git command failed: {}", stderr);
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn branch_exists(repo_root: &Path, branch: &str) -> Result<bool> {
    let output = Command::new("git")
        .current_dir(repo_root)
        .args([
            "show-ref",
            "--verify",
            &format!("refs/heads/{}", branch),
        ])
        .output()?;
    Ok(output.status.success())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn init_git_repo(dir: &TempDir) {
        let _ = Command::new("git")
            .current_dir(dir.path())
            .args(["init", "-b", "main"])
            .output();
        let _ = Command::new("git")
            .current_dir(dir.path())
            .args([
                "config",
                "user.email",
                "test@test.com",
            ])
            .output();
        let _ = Command::new("git")
            .current_dir(dir.path())
            .args([
                "config",
                "user.name",
                "Test",
            ])
            .output();
        fs::write(dir.path().join("init.txt"), "init\n").unwrap();
        let _ = Command::new("git")
            .current_dir(dir.path())
            .args([
                "add",
                "init.txt",
            ])
            .output();
        let _ = Command::new("git")
            .current_dir(dir.path())
            .args([
                "commit",
                "-m",
                "init",
            ])
            .output();
    }

    #[test]
    fn test_not_git_repo() {
        let tmp = TempDir::new().unwrap();
        let _ = std::env::set_current_dir(tmp.path());
        let result = Git::new();
        assert!(result.is_err());
    }

    #[test]
    fn test_branch_creation_and_switch() {
        let tmp = TempDir::new().unwrap();
        init_git_repo(&tmp);
        let git = Git::from_dir(tmp.path().to_path_buf()).unwrap();
        let branch = git.create_and_switch_branch("20260508-1200").unwrap();
        assert_eq!(branch, "code-ant/20260508-1200");
        assert_eq!(git.current_branch().unwrap(), "code-ant/20260508-1200");
    }

    #[test]
    fn test_branch_collision_avoidance() {
        let tmp = TempDir::new().unwrap();
        init_git_repo(&tmp);
        let git = Git::from_dir(tmp.path().to_path_buf()).unwrap();
        let _ = git.create_and_switch_branch("20260508-1200").unwrap();
        let git2 = Git::from_dir(tmp.path().to_path_buf()).unwrap();
        let branch = git2.create_and_switch_branch("20260508-1200").unwrap();
        assert_eq!(branch, "code-ant/20260508-1200-2");
    }

    #[test]
    fn test_stage_and_commit() {
        let tmp = TempDir::new().unwrap();
        init_git_repo(&tmp);
        let git = Git::from_dir(tmp.path().to_path_buf()).unwrap();
        let _ = git.create_and_switch_branch("20260508-1201").unwrap();
        let file = tmp.path().join("hello.py");
        fs::write(&file, "x = 1\n").unwrap();
        git.stage_and_commit(&file, "[code-ant] test in hello.py").unwrap();
        let log = run_git(tmp.path(), &["log", "--oneline"]).unwrap();
        assert!(log.contains("[code-ant]"));
    }

    #[test]
    fn test_verify_clean_detects_dirty() {
        let tmp = TempDir::new().unwrap();
        init_git_repo(&tmp);
        let git = Git::from_dir(tmp.path().to_path_buf()).unwrap();
        // Modify tracked file to make working tree dirty
        let file = tmp.path().join("init.txt");
        fs::write(&file, "dirty\n").unwrap();
        let result = git.verify_clean();
        assert!(result.is_err());
    }

    #[test]
    fn test_reset_hard_cleans_tracked() {
        let tmp = TempDir::new().unwrap();
        init_git_repo(&tmp);
        let git = Git::from_dir(tmp.path().to_path_buf()).unwrap();
        // Create tracked file, modify it
        let file = tmp.path().join("tracked.txt");
        fs::write(&file, "original\n").unwrap();
        let _ = Command::new("git")
            .current_dir(tmp.path())
            .args(["add", "tracked.txt"])
            .output();
        let _ = Command::new("git")
            .current_dir(tmp.path())
            .args(["commit", "-m", "track"])
            .output();
        fs::write(&file, "modified\n").unwrap();
        git.reset_hard_to_head().unwrap();
        let content = fs::read_to_string(&file).unwrap();
        assert_eq!(content, "original\n");
        assert!(!git.is_dirty().unwrap());
    }

    #[test]
    fn test_checkout_file_restores() {
        let tmp = TempDir::new().unwrap();
        init_git_repo(&tmp);
        let git = Git::from_dir(tmp.path().to_path_buf()).unwrap();
        let file = tmp.path().join("tracked.txt");
        fs::write(&file, "original\n").unwrap();
        let _ = Command::new("git")
            .current_dir(tmp.path())
            .args(["add", "tracked.txt"])
            .output();
        let _ = Command::new("git")
            .current_dir(tmp.path())
            .args(["commit", "-m", "track"])
            .output();
        fs::write(&file, "modified\n").unwrap();
        git.checkout_file(&file).unwrap();
        let content = fs::read_to_string(&file).unwrap();
        assert_eq!(content, "original\n");
    }
}
