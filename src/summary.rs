#[derive(Debug, Clone)]
pub struct RunSummary {
    pub commits: u64,
    pub distinct_files: Vec<String>,
    pub elapsed_secs: f64,
}

impl RunSummary {
    pub fn print(&self) {
        let files: Vec<String> = self.distinct_files.iter().cloned().collect();
        println!("=== code-ant summary ===");
        println!("Commits created: {}", self.commits);
        println!("Distinct files modified: {}", files.len());
        if !files.is_empty() {
            println!("Files:");
            for f in &files {
                println!("  - {}", f);
            }
        }
        println!("Elapsed time: {:.2}s", self.elapsed_secs);
        println!("========================");
    }
}
