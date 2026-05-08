use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;
use anyhow::{Result, bail, Context};
use chrono::Local;

use crate::cli::{Args, parse_duration};
use crate::config::{Config, resolve_provider_config};
use crate::crawler::Crawler;
use crate::git::Git;
use crate::models::{ProviderParams, build_provider};
use crate::summary::RunSummary;
use crate::test_runner::TestRunner;
use crate::transformer::{Transformer, TransformCandidate};

pub struct Engine {
    args: Args,
    git: Git,
    crawler: Crawler,
    transformer: Transformer,
    test_runner: Option<TestRunner>,
    start_time: Instant,
    max_time_secs: Option<u64>,
    shutdown: Arc<AtomicBool>,
    created_branch: Option<String>,
    dirty: bool,
}

impl Engine {
    pub async fn new(
        args: Args,
        cfg: Config,
        shutdown: Arc<AtomicBool>,
    ) -> Result<Self> {
        let git = Git::new()?;
        let repo_root = git.repo_root().to_path_buf();

        let current_branch = git.current_branch()?;
        let created_branch = if current_branch == "main" || current_branch == "master" {
            let timestamp = Local::now().format("%Y%m%d-%H%M").to_string();
            Some(git.create_and_switch_branch(&timestamp)?)
        } else {
            None
        };

        let include_globs = args.include.as_ref().map(|v| v.to_vec());
        let exclude_globs = args.exclude.as_ref().map(|v| v.to_vec());
        let crawler = Crawler::new(
            repo_root.clone(),
            include_globs.as_ref(),
            exclude_globs.as_ref(),
        )?;

        let provider = build_provider_from_config(&args, &cfg)?;
        let transformer = Transformer::new(provider);

        let test_runner = if args.skip_tests {
            None
        } else {
            let tr = TestRunner::detect(&repo_root, args.test_cmd.clone())?;
            if tr.is_none() {
                bail!(crate::error::CodeAntError::NoTestCommand);
            }
            tr
        };

        let max_time_secs = args.max_time.as_ref().map(|s| parse_duration(s)).transpose()?;

        if args.skip_tests {
            eprintln!("WARNING: --skip-tests is active. Test verification is disabled. Changes may be committed without validation.");
        }

        Ok(Engine {
            args,
            git,
            crawler,
            transformer,
            test_runner,
            start_time: Instant::now(),
            max_time_secs,
            shutdown,
            created_branch,
            dirty: false,
        })
    }

    pub async fn run(&mut self) -> Result<RunSummary> {
        let files = self.crawler.crawl()?;
        let mut commits = 0u64;
        let mut distinct_files: HashSet<String> = HashSet::new();

        for file in &files {
            if self.should_stop(commits, &distinct_files) {
                break;
            }
            if self.shutdown.load(Ordering::SeqCst) {
                break;
            }

            if let Err(e) = self.git.verify_clean() {
                self.rollback()?;
                bail!("Working tree or HEAD diverged before iteration: {}", e);
            }

            let rel_path = file.strip_prefix(self.git.repo_root()).unwrap_or(file);
            let rel_str = rel_path.to_string_lossy().to_string();
            let language = language_from_extension(file);

            let content = match std::fs::read_to_string(file) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("Skipping {}: cannot read: {}", rel_str, e);
                    continue;
                }
            };

            let candidates = match self.transformer.generate_candidates(
                &rel_str, &content, &language,
            ).await {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("Skipping {}: model error: {}", rel_str, e);
                    continue;
                }
            };

            for candidate in candidates {
                if self.should_stop(commits, &distinct_files) {
                    break;
                }
                if self.shutdown.load(Ordering::SeqCst) {
                    break;
                }
                if let Err(e) = self.git.verify_clean() {
                    self.rollback()?;
                    bail!("Working tree or HEAD diverged mid-run: {}", e);
                }

                if self.args.dry_run {
                    println!("--- Dry-run candidate ---");
                    println!("File: {}", candidate.file_path);
                    println!("Message: {}", commit_message(&candidate));
                    if !candidate.diff.is_empty() {
                        println!("Diff:\n{}", candidate.diff);
                    } else {
                        println!("New content:\n{}", candidate.new_content);
                    }
                    println!("---");
                    continue;
                }

                // Apply transformation
                self.dirty = true;
                std::fs::write(file, &candidate.new_content)
                    .with_context(|| format!("Writing {}", rel_str))?;

                // Syntax validation
                if !is_syntax_valid(file, &language) {
                    eprintln!("Skipping {}: transformation produced invalid syntax", rel_str);
                    self.git.checkout_file(file)?;
                    self.dirty = false;
                    continue;
                }

                // Run tests
                let tests_pass = if let Some(runner) = &self.test_runner {
                    runner.run()?
                } else {
                    true
                };

                if !tests_pass {
                    eprintln!("Tests failed for {}. Rolling back.", rel_str);
                    self.git.checkout_file(file)?;
                    self.dirty = false;
                    continue;
                }

                // Commit
                let msg = commit_message(&candidate);
                self.git.stage_and_commit(file, &msg)?;
                self.git.update_expected_head()?;
                self.dirty = false;

                commits += 1;
                distinct_files.insert(rel_str.clone());
            }
        }

        let summary = RunSummary {
            commits,
            distinct_files: distinct_files.into_iter().collect(),
            elapsed_secs: self.start_time.elapsed().as_secs_f64(),
        };
        Ok(summary)
    }

    fn should_stop(&self, commits: u64, distinct_files: &HashSet<String>) -> bool {
        if let Some(max) = self.args.max_commits {
            if commits >= max {
                return true;
            }
        }
        if let Some(max) = self.args.max_files {
            if distinct_files.len() as u64 >= max {
                return true;
            }
        }
        if let Some(max_secs) = self.max_time_secs {
            let elapsed = self.start_time.elapsed().as_secs();
            if elapsed >= max_secs {
                return true;
            }
        }
        false
    }

    pub fn cleanup(&self) -> Result<()> {
        if self.dirty {
            self.git.reset_hard_to_head()?;
        }
        Ok(())
    }

    fn rollback(&self) -> Result<()> {
        self.git.reset_hard_to_head()?;
        Ok(())
    }
}

fn build_provider_from_config(args: &Args, cfg: &Config) -> Result<crate::models::ModelProvider> {
    let (provider_name, model, host, api_key) = resolve_provider_config(
        cfg,
        args.provider.as_deref(),
        args.model.as_deref(),
        args.api_key.as_deref(),
    );
    let params = ProviderParams {
        host: host.unwrap_or_default(),
        api_key,
        model,
    };
    build_provider(&provider_name, params)
}

fn language_from_extension(path: &std::path::Path) -> String {
    path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_string()
}

fn commit_message(candidate: &TransformCandidate) -> String {
    format!(
        "[code-ant] {} in {}",
        candidate.description, candidate.file_path
    )
}

fn is_syntax_valid(path: &std::path::Path, language: &str) -> bool {
    match language {
        "py" => {
            let output = std::process::Command::new("python3")
                .args(["-m", "py_compile", path.to_str().unwrap_or("")])
                .output();
            match output {
                Ok(o) => o.status.success(),
                Err(_) => true,
            }
        }
        _ => true,
    }
}
