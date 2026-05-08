use thiserror::Error;

#[derive(Error, Debug)]
pub enum CodeAntError {
    #[error("Not inside a git repository")]
    NotGitRepo,
    #[error("Working tree is dirty")]
    DirtyWorkingTree,
    #[error("HEAD has diverged from recorded state")]
    DivergentHead,
    #[error("No test command found. Provide --test-cmd or use --skip-tests")]
    NoTestCommand,
    #[error("Branch creation failed: {0}")]
    BranchCreationFailed(String),
    #[error("Invalid duration format: {0}")]
    InvalidDuration(String),
    #[error("Transform produced invalid syntax")]
    InvalidSyntax,
    #[error("Model provider error: {0}")]
    ModelProvider(String),
}
