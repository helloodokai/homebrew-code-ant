use anyhow::Result;
use globset::{Glob, GlobSet, GlobSetBuilder};
use std::path::PathBuf;
use walkdir::WalkDir;

const DEFAULT_EXTENSIONS: [&'static str; 11] = [
    "py", "js", "ts", "go", "java", "cpp", "c", "rs", "tsx", "jsx", "mjs",
];

pub struct Crawler {
    repo_root: PathBuf,
    include: Option<GlobSet>,
    exclude: GlobSet,
}

impl Crawler {
    pub fn new(
        repo_root: PathBuf,
        include_globs: Option<&Vec<String>>,
        exclude_globs: Option<&Vec<String>>,
    ) -> Result<Self> {
        let include = include_globs.map(|globs| build_globset(globs)).transpose()?;
        let mut exclude_builder = GlobSetBuilder::new();
        // Always exclude .git, node_modules, target, etc.
        for pat in [
            "**/.git/**",
            "**/node_modules/**",
            "**/target/**",
            "**/__pycache__/**",
            "**/*.egg-info/**",
            "**/vendor/**",
            "**/dist/**",
            "**/build/**",
        ] {
            exclude_builder.add(Glob::new(pat)?);
        }
        if let Some(globs) = exclude_globs {
            for g in globs {
                exclude_builder.add(Glob::new(g)?);
            }
        }
        let exclude = exclude_builder.build()?;
        Ok(Crawler { repo_root, include, exclude })
    }

    pub fn crawl(&self) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();
        for entry in WalkDir::new(&self.repo_root)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let rel = path.strip_prefix(&self.repo_root).unwrap_or(path);
            let rel_str = rel.to_string_lossy().replace('\\', "/");
            if self.exclude.is_match(&rel_str) {
                continue;
            }
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("");
            let matches_ext = DEFAULT_EXTENSIONS.contains(&ext);
            let matches_include = self
                .include
                .as_ref()
                .map(|gs| gs.is_match(&rel_str))
                .unwrap_or(matches_ext);
            if matches_include {
                files.push(path.to_path_buf());
            }
        }
        files.sort();
        Ok(files)
    }
}

fn build_globset(globs: &Vec<String>) -> Result<GlobSet> {
    let mut builder = GlobSetBuilder::new();
    for g in globs {
        builder.add(Glob::new(g)?);
    }
    Ok(builder.build()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_crawl_default_extensions() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().to_path_buf();
        fs::write(root.join("a.py"), "x = 1\n").unwrap();
        fs::write(root.join("b.rs"), "fn main() {}\n").unwrap();
        fs::write(root.join("c.txt"), "hello\n").unwrap();
        let c = Crawler::new(root.clone(), None, None).unwrap();
        let files = c.crawl().unwrap();
        let names: Vec<String> = files
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert!(names.contains(&"a.py".to_string()));
        assert!(names.contains(&"b.rs".to_string()));
        assert!(!names.contains(&"c.txt".to_string()));
    }

    #[test]
    fn test_crawl_include_glob() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().to_path_buf();
        fs::write(root.join("a.py"), "x = 1\n").unwrap();
        fs::write(root.join("b.rs"), "fn main() {}\n").unwrap();
        let c = Crawler::new(root.clone(), Some(&vec!["**/*.py".to_string()]), None).unwrap();
        let files = c.crawl().unwrap();
        let names: Vec<String> = files
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert!(names.contains(&"a.py".to_string()));
        assert!(!names.contains(&"b.rs".to_string()));
    }

    #[test]
    fn test_crawl_exclude_glob() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().to_path_buf();
        fs::write(root.join("a.py"), "x = 1\n").unwrap();
        fs::write(root.join("b.rs"), "fn main() {}\n").unwrap();
        let c = Crawler::new(root.clone(), None, Some(&vec!["**/*.rs".to_string()])).unwrap();
        let files = c.crawl().unwrap();
        let names: Vec<String> = files
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert!(names.contains(&"a.py".to_string()));
        assert!(!names.contains(&"b.rs".to_string()));
    }

    #[test]
    fn test_crawl_exclude_precedence() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().to_path_buf();
        fs::write(root.join("a.py"), "x = 1\n").unwrap();
        fs::write(root.join("b.rs"), "fn main() {}\n").unwrap();
        let c = Crawler::new(
            root.clone(),
            Some(&vec!["**/*".to_string()]),
            Some(&vec!["**/*.py".to_string()]),
        )
        .unwrap();
        let files = c.crawl().unwrap();
        let names: Vec<String> = files
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert!(!names.contains(&"a.py".to_string()));
        assert!(names.contains(&"b.rs".to_string()));
    }
}
