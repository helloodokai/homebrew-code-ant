use std::process::Command;
use std::fs;
use tempfile::TempDir;

fn init_git_repo(dir: &TempDir) {
    let _ = Command::new("git")
        .current_dir(dir.path())
        .args(["init", "-b", "main"])
        .output()
        .expect("git init failed");
    let _ = Command::new("git")
        .current_dir(dir.path())
        .args(["config", "user.email", "test@test.com"])
        .output();
    let _ = Command::new("git")
        .current_dir(dir.path())
        .args(["config", "user.name", "Test"])
        .output();
    fs::write(dir.path().join("init.txt"), "init\n").unwrap();
    let _ = Command::new("git")
        .current_dir(dir.path())
        .args(["add", "init.txt"])
        .output();
    let _ = Command::new("git")
        .current_dir(dir.path())
        .args(["commit", "-m", "init"])
        .output();
}

fn git_stdout(dir: &TempDir, args: &[&str]) -> String {
    String::from_utf8_lossy(
        &Command::new("git")
            .current_dir(dir.path())
            .args(args)
            .output()
            .unwrap()
            .stdout,
    )
    .to_string()
}

fn mock_json_response(description: &str, new_content: &str) -> String {
    let value = serde_json::json!([{
        "description": description,
        "diff": "",
        "new_content": new_content,
    }]);
    value.to_string()
}

#[test]
fn test_ac_11_not_git_repo() {
    let tmp = TempDir::new().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_code-ant"))
        .current_dir(tmp.path())
        .args(["--skip-tests"])
        .output()
        .expect("failed to execute");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Not inside a git repository"));
}

#[test]
fn test_ac_1_creates_branch_on_main() {
    let tmp = TempDir::new().unwrap();
    init_git_repo(&tmp);
    fs::write(tmp.path().join("a.py"), "import os\nimport sys\n\nprint(sys.version)\n").unwrap();
    let _ = Command::new("git")
        .current_dir(tmp.path())
        .args(["add", "a.py"])
        .output();
    let _ = Command::new("git")
        .current_dir(tmp.path())
        .args(["commit", "-m", "add py"])
        .output();

    let response = mock_json_response("Remove unused import", "import sys\nprint(sys.version)\n");
    let output = Command::new(env!("CARGO_BIN_EXE_code-ant"))
        .current_dir(tmp.path())
        .env("CODE_ANT_PROVIDER", "mock")
        .env("CODE_ANT_MODEL", &response)
        .args(["--skip-tests", "--max-commits", "1"])
        .output()
        .expect("failed to execute");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}", stdout, stderr
    );

    let branches = git_stdout(&tmp, &["branch", "--list", "code-ant/*"]);
    assert!(branches.contains("code-ant/"));
    let current = git_stdout(&tmp, &["rev-parse", "--abbrev-ref", "HEAD"]);
    assert!(current.contains("code-ant/"));
}

#[test]
fn test_ac_2_no_new_branch_on_feature() {
    let tmp = TempDir::new().unwrap();
    init_git_repo(&tmp);
    let _ = Command::new("git")
        .current_dir(tmp.path())
        .args(["checkout", "-b", "feature"])
        .output();
    fs::write(tmp.path().join("a.py"), "import os\nimport sys\n\nprint(sys.version)\n").unwrap();
    let _ = Command::new("git")
        .current_dir(tmp.path())
        .args(["add", "a.py"])
        .output();
    let _ = Command::new("git")
        .current_dir(tmp.path())
        .args(["commit", "-m", "add py"])
        .output();

    let response = mock_json_response("Remove unused import", "import sys\nprint(sys.version)\n");
    let output = Command::new(env!("CARGO_BIN_EXE_code-ant"))
        .current_dir(tmp.path())
        .env("CODE_ANT_PROVIDER", "mock")
        .env("CODE_ANT_MODEL", &response)
        .args(["--skip-tests", "--max-commits", "1"])
        .output()
        .expect("failed to execute");
    assert!(output.status.success());
    let current = git_stdout(&tmp, &["rev-parse", "--abbrev-ref", "HEAD"]);
    assert_eq!(current.trim(), "feature");
    let branches = git_stdout(&tmp, &["branch"]);
    assert!(!branches.contains("code-ant/"));
}

#[test]
fn test_ac_3_max_commits() {
    let tmp = TempDir::new().unwrap();
    init_git_repo(&tmp);
    fs::write(tmp.path().join("a.py"), "import os\nimport sys\n\nprint(sys.version)\n").unwrap();
    fs::write(tmp.path().join("b.py"), "import json\n\nprint('hello')\n").unwrap();
    fs::write(tmp.path().join("c.py"), "import math\n\nprint(math.pi)\n").unwrap();
    let _ = Command::new("git")
        .current_dir(tmp.path())
        .args(["add", "."])
        .output();
    let _ = Command::new("git")
        .current_dir(tmp.path())
        .args(["commit", "-m", "add files"])
        .output();

    let response = mock_json_response("Remove unused import", "import sys\nprint(sys.version)\n");
    let output = Command::new(env!("CARGO_BIN_EXE_code-ant"))
        .current_dir(tmp.path())
        .env("CODE_ANT_PROVIDER", "mock")
        .env("CODE_ANT_MODEL", &response)
        .args(["--skip-tests", "--max-commits", "3"])
        .output()
        .expect("failed to execute");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}", stdout, stderr
    );
    let log = git_stdout(&tmp, &["log", "--oneline", "--grep=\\[code-ant\\]"]);
    let count = log.lines().count();
    assert_eq!(count, 3, "Expected 3 code-ant commits, got {}. Log:\n{}", count, log);
}

#[test]
fn test_ac_4_max_files() {
    let tmp = TempDir::new().unwrap();
    init_git_repo(&tmp);
    fs::write(tmp.path().join("a.py"), "import os\nimport sys\n\nprint(sys.version)\n").unwrap();
    fs::write(tmp.path().join("b.py"), "import json\n\nprint('hello')\n").unwrap();
    fs::write(tmp.path().join("c.py"), "import math\n\nprint(math.pi)\n").unwrap();
    let _ = Command::new("git")
        .current_dir(tmp.path())
        .args(["add", "."])
        .output();
    let _ = Command::new("git")
        .current_dir(tmp.path())
        .args(["commit", "-m", "add files"])
        .output();

    let response = mock_json_response("Remove unused import", "import sys\nprint(sys.version)\n");
    let output = Command::new(env!("CARGO_BIN_EXE_code-ant"))
        .current_dir(tmp.path())
        .env("CODE_ANT_PROVIDER", "mock")
        .env("CODE_ANT_MODEL", &response)
        .args(["--skip-tests", "--max-files", "2"])
        .output()
        .expect("failed to execute");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}", stdout, stderr
    );
    let log = git_stdout(&tmp, &["log", "--format=%s"]);
    let distinct_files: std::collections::HashSet<&str> = log
        .lines()
        .filter(|l| l.starts_with("[code-ant]"))
        .filter_map(|l| l.rsplit(" in ").next())
        .collect();
    assert_eq!(
        distinct_files.len(),
        2,
        "Expected 2 distinct files, got {:?}",
        distinct_files
    );
}

#[test]
fn test_ac_6_rollback_on_test_failure() {
    let tmp = TempDir::new().unwrap();
    init_git_repo(&tmp);
    fs::write(tmp.path().join("a.py"), "import os\nimport sys\n\nprint(sys.version)\n").unwrap();
    fs::write(
        tmp.path().join("test_a.py"),
        "import pytest\n\ndef test_dummy():\n    assert True\n",
    )
    .unwrap();
    let _ = Command::new("git")
        .current_dir(tmp.path())
        .args(["add", "."])
        .output();
    let _ = Command::new("git")
        .current_dir(tmp.path())
        .args(["commit", "-m", "add files"])
        .output();

    let response = mock_json_response("Remove unused import", "import sys\nprint(sys.version)\n");
    let output = Command::new(env!("CARGO_BIN_EXE_code-ant"))
        .current_dir(tmp.path())
        .env("CODE_ANT_PROVIDER", "mock")
        .env("CODE_ANT_MODEL", &response)
        .args(["--test-cmd", "false", "--max-commits", "1"])
        .output()
        .expect("failed to execute");
    assert!(output.status.success());
    let log = git_stdout(&tmp, &["log", "--oneline", "--grep=\\[code-ant\\]"]);
    assert!(
        log.is_empty(),
        "Expected no commits when tests fail, got: {}",
        log
    );
    let status = git_stdout(&tmp, &["status", "--porcelain", "--untracked-files=no"]);
    assert!(
        status.trim().is_empty(),
        "Working tree should be clean, got: {}",
        status
    );
}

#[test]
fn test_ac_7_commit_message_format() {
    let tmp = TempDir::new().unwrap();
    init_git_repo(&tmp);
    fs::write(tmp.path().join("a.py"), "import os\nimport sys\n\nprint(sys.version)\n").unwrap();
    let _ = Command::new("git")
        .current_dir(tmp.path())
        .args(["add", "."])
        .output();
    let _ = Command::new("git")
        .current_dir(tmp.path())
        .args(["commit", "-m", "add files"])
        .output();

    let response = mock_json_response("Remove unused import", "import sys\nprint(sys.version)\n");
    let output = Command::new(env!("CARGO_BIN_EXE_code-ant"))
        .current_dir(tmp.path())
        .env("CODE_ANT_PROVIDER", "mock")
        .env("CODE_ANT_MODEL", &response)
        .args(["--skip-tests", "--max-commits", "1"])
        .output()
        .expect("failed to execute");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "Binary failed. stdout: {}\nstderr: {}", stdout, stderr
    );
    let log = git_stdout(&tmp, &["log", "--format=%s"]);
    let msg = log
        .lines()
        .find(|l| l.starts_with("[code-ant]"))
        .expect("no code-ant commit");
    assert!(msg.contains(" in "), "Commit message should contain ' in ': {}", msg);
    assert!(
        msg != "[code-ant]  in " && msg != "[code-ant] in ",
        "Commit message should have non-empty description and path: {}",
        msg
    );
}

#[test]
fn test_ac_8_dry_run_no_changes() {
    let tmp = TempDir::new().unwrap();
    init_git_repo(&tmp);
    fs::write(tmp.path().join("a.py"), "import os\nimport sys\n\nprint(sys.version)\n").unwrap();
    let _ = Command::new("git")
        .current_dir(tmp.path())
        .args(["add", "."])
        .output();
    let _ = Command::new("git")
        .current_dir(tmp.path())
        .args(["commit", "-m", "add files"])
        .output();

    let before_log = git_stdout(&tmp, &["log", "--format=%H"]);

    let response = mock_json_response("Remove unused import", "import sys\nprint(sys.version)\n");
    let output = Command::new(env!("CARGO_BIN_EXE_code-ant"))
        .current_dir(tmp.path())
        .env("CODE_ANT_PROVIDER", "mock")
        .env("CODE_ANT_MODEL", &response)
        .args(["--dry-run", "--skip-tests"])
        .output()
        .expect("failed to execute");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Dry-run candidate"),
        "Expected dry-run output"
    );

    let after_log = git_stdout(&tmp, &["log", "--format=%H"]);
    assert_eq!(
        before_log, after_log,
        "Log should be unchanged in dry-run"
    );

    let status = git_stdout(&tmp, &["status", "--porcelain"]);
    assert!(
        status.trim().is_empty(),
        "Working tree should be clean after dry-run"
    );
}

#[test]
fn test_ac_10_include_glob() {
    let tmp = TempDir::new().unwrap();
    init_git_repo(&tmp);
    fs::write(tmp.path().join("a.py"), "import os\n\nprint('py')\n").unwrap();
    fs::write(tmp.path().join("b.rs"), "fn main() {}\n").unwrap();
    let _ = Command::new("git")
        .current_dir(tmp.path())
        .args(["add", "."])
        .output();
    let _ = Command::new("git")
        .current_dir(tmp.path())
        .args(["commit", "-m", "add files"])
        .output();

    let response = mock_json_response("Remove unused import", "\nprint('py')\n");
    let output = Command::new(env!("CARGO_BIN_EXE_code-ant"))
        .current_dir(tmp.path())
        .env("CODE_ANT_PROVIDER", "mock")
        .env("CODE_ANT_MODEL", &response)
        .args(["--skip-tests", "--include", "**/*.py", "--max-commits", "1"])
        .output()
        .expect("failed to execute");
    assert!(output.status.success());
    let log = git_stdout(&tmp, &["log", "--format=%s"]);
    let commits: Vec<&str> = log.lines().filter(|l| l.starts_with("[code-ant]")).collect();
    for commit in commits {
        let file = commit.rsplit(" in ").next().unwrap_or("");
        assert!(
            file.ends_with(".py"),
            "Commit should only touch .py files: {}",
            commit
        );
    }
}

#[test]
fn test_ac_12_no_test_cmd_warns() {
    let tmp = TempDir::new().unwrap();
    init_git_repo(&tmp);
    fs::write(tmp.path().join("a.py"), "x = 1\n").unwrap();
    let _ = Command::new("git")
        .current_dir(tmp.path())
        .args(["add", "."])
        .output();
    let _ = Command::new("git")
        .current_dir(tmp.path())
        .args(["commit", "-m", "init"])
        .output();

    let output = Command::new(env!("CARGO_BIN_EXE_code-ant"))
        .current_dir(tmp.path())
        .env("CODE_ANT_PROVIDER", "mock")
        .env("CODE_ANT_MODEL", "[]")
        .args(["--max-commits", "1"])
        .output()
        .expect("failed to execute");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("No test command")
            || stderr.contains("Provide --test-cmd")
            || stderr.contains("skip-tests"),
        "stderr: {}",
        stderr
    );
}
