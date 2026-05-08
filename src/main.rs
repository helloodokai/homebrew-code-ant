use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use anyhow::Result;
use clap::Parser;
use futures::StreamExt;

mod cli;
mod config;
mod crawler;
mod engine;
mod error;
mod git;
mod models;
mod summary;
mod test_runner;
mod transformer;

#[tokio::main]
async fn main() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .try_init();

    let args = cli::Args::parse();
    let exit_code = match run(args).await {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("code-ant: error: {}", e);
            1
        }
    };
    std::process::exit(exit_code);
}

async fn run(args: cli::Args) -> Result<()> {
    let shutdown = Arc::new(AtomicBool::new(false));
    let shutdown_clone = shutdown.clone();

    let mut signals = signal_hook_tokio::Signals::new([
        signal_hook::consts::SIGINT,
        signal_hook::consts::SIGTERM,
    ])?;

    tokio::spawn(async move {
        while let Some(_sig) = signals.next().await {
            shutdown_clone.store(true, Ordering::SeqCst);
        }
    });

    let cfg = config::load_config()?;
    let mut engine = engine::Engine::new(args, cfg, shutdown).await?;
    let result = engine.run().await;

    match result {
        Ok(summary) => {
            summary.print();
            Ok(())
        }
        Err(e) => {
            tracing::error!("Run failed: {}", e);
            let _ = engine.cleanup();
            Err(e)
        }
    }
}
