use clap::{Parser, ArgAction};

#[derive(Parser, Debug, Clone)]
#[command(name = "code-ant", version, about = "Autonomous code improvement agent")]
#[command(after_help = "LIMITATION: If your project's test command returns exit code 0 even on failure, test verification will be unreliable. In that case, supply --test-cmd with a command that accurately reflects pass/fail.")]
pub struct Args {
    #[arg(long, help = "Maximum number of successful commits")]
    pub max_commits: Option<u64>,

    #[arg(long, help = "Maximum number of distinct files modified")]
    pub max_files: Option<u64>,

    #[arg(long, help = "Maximum elapsed wall-clock time (e.g., 30m, 2h)")]
    pub max_time: Option<String>,

    #[arg(long, action = ArgAction::Append, help = "Include only files matching glob")]
    pub include: Option<Vec<String>>,

    #[arg(long, action = ArgAction::Append, help = "Exclude files matching glob")]
    pub exclude: Option<Vec<String>>,

    #[arg(long, help = "Print proposed changes without applying them")]
    pub dry_run: bool,

    #[arg(long, help = "Disable test suite execution")]
    pub skip_tests: bool,

    #[arg(long, help = "Override auto-detected test command")]
    pub test_cmd: Option<String>,

    #[arg(long, help = "Override AI model provider")]
    pub provider: Option<String>,

    #[arg(long, help = "Override AI model name")]
    pub model: Option<String>,

    #[arg(long, help = "API key for model provider")]
    pub api_key: Option<String>,
}

pub fn parse_duration(s: &str) -> anyhow::Result<u64> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        anyhow::bail!("Duration is empty");
    }
    let suffix = trimmed.chars().last().unwrap();
    let has_unit = !suffix.is_ascii_digit();
    if has_unit {
        let num_part = &trimmed[..trimmed.len() - 1];
        let num: u64 = num_part
            .parse()
            .map_err(|_| anyhow::anyhow!("Invalid duration number: {}", num_part))?;
        match suffix {
            's' | 'S' => Ok(num),
            'm' | 'M' => Ok(num * 60),
            'h' | 'H' => Ok(num * 60 * 60),
            _ => anyhow::bail!("Invalid duration suffix: {}. Use s, m, or h.", suffix),
        }
    } else {
        let num: u64 = trimmed
            .parse()
            .map_err(|_| anyhow::anyhow!("Invalid duration: {}", s))?;
        Ok(num * 60)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_duration_seconds() {
        assert_eq!(parse_duration("10s").unwrap(), 10);
        assert_eq!(parse_duration("10S").unwrap(), 10);
    }

    #[test]
    fn test_parse_duration_minutes() {
        assert_eq!(parse_duration("30m").unwrap(), 1800);
        assert_eq!(parse_duration("30M").unwrap(), 1800);
    }

    #[test]
    fn test_parse_duration_hours() {
        assert_eq!(parse_duration("2h").unwrap(), 7200);
        assert_eq!(parse_duration("2H").unwrap(), 7200);
    }

    #[test]
    fn test_parse_duration_bare_number() {
        assert_eq!(parse_duration("5").unwrap(), 300);
    }

    #[test]
    fn test_parse_duration_invalid() {
        assert!(parse_duration("abc").is_err());
        assert!(parse_duration("10x").is_err());
        assert!(parse_duration("").is_err());
    }
}
