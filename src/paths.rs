use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Harness {
    Claude,
    Pi,
}

impl Harness {
    pub const ALL: [Harness; 2] = [Harness::Claude, Harness::Pi];

    pub fn name(self) -> &'static str {
        match self {
            Harness::Claude => "claude",
            Harness::Pi => "pi",
        }
    }

    pub fn parse(s: &str) -> Option<Harness> {
        match s {
            "claude" => Some(Harness::Claude),
            "pi" => Some(Harness::Pi),
            _ => None,
        }
    }
}

/// Every location tack reads or writes. All of them derive from `home`, so tests pass a temp dir.
#[derive(Clone, Debug)]
pub struct Paths {
    pub home: PathBuf,
    pub tack: PathBuf,
    pub claude: PathBuf,
    pub pi: PathBuf,
    pub t3: PathBuf,
}

impl Paths {
    pub fn from_env() -> Result<Self> {
        let home = PathBuf::from(std::env::var_os("HOME").context("HOME is not set")?);
        let mut paths = Self::from_home(&home);
        if let Some(dir) = non_empty_env("TACK_HOME") {
            paths.tack = dir;
        }
        if let Some(dir) = non_empty_env("CLAUDE_CONFIG_DIR") {
            paths.claude = dir;
        }
        if let Some(dir) = non_empty_env("T3CODE_HOME") {
            paths.t3 = dir;
        }
        Ok(paths)
    }

    pub fn from_home(home: &Path) -> Self {
        Self {
            home: home.to_path_buf(),
            tack: home.join(".tack"),
            claude: home.join(".claude"),
            pi: home.join(".pi").join("agent"),
            t3: home.join(".t3"),
        }
    }

    pub fn tack_bin(&self) -> PathBuf {
        self.tack.join("bin").join("tack")
    }

    pub fn local_bin(&self) -> PathBuf {
        self.home.join(".local").join("bin")
    }

    pub fn store(&self) -> PathBuf {
        self.tack.join("store")
    }

    pub fn state_file(&self) -> PathBuf {
        self.tack.join("state.toml")
    }

    pub fn user_manifest(&self) -> PathBuf {
        self.tack.join("tack.toml")
    }

    pub fn backup_dir(&self) -> PathBuf {
        self.tack.join("backup")
    }

    pub fn config_dir(&self, harness: Harness) -> &Path {
        match harness {
            Harness::Claude => &self.claude,
            Harness::Pi => &self.pi,
        }
    }

    pub fn skills_dir(&self, harness: Harness) -> PathBuf {
        self.config_dir(harness).join("skills")
    }

    pub fn claude_settings(&self) -> PathBuf {
        self.claude.join("settings.json")
    }

    pub fn t3_settings(&self) -> PathBuf {
        self.t3.join("userdata").join("settings.json")
    }

    /// The file the agent loads in every session; holds the `rules` block.
    pub fn rules_file(&self, harness: Harness) -> PathBuf {
        match harness {
            Harness::Claude => self.claude.join("CLAUDE.md"),
            Harness::Pi => self.pi.join("AGENTS.md"),
        }
    }

    pub fn pi_rules_extension(&self) -> PathBuf {
        self.pi.join("extensions").join("tack-rules.ts")
    }

    pub fn pi_extension(&self) -> PathBuf {
        self.pi.join("extensions").join("tack-guard.ts")
    }
}

fn non_empty_env(key: &str) -> Option<PathBuf> {
    std::env::var_os(key)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn everything_derives_from_home() {
        let p = Paths::from_home(Path::new("/h"));
        assert_eq!(p.tack_bin(), Path::new("/h/.tack/bin/tack"));
        assert_eq!(
            p.skills_dir(Harness::Claude),
            Path::new("/h/.claude/skills")
        );
        assert_eq!(p.skills_dir(Harness::Pi), Path::new("/h/.pi/agent/skills"));
        assert_eq!(
            p.pi_extension(),
            Path::new("/h/.pi/agent/extensions/tack-guard.ts")
        );
        assert_eq!(Harness::parse("pi"), Some(Harness::Pi));
        assert_eq!(Harness::parse("codex"), None);
    }
}
