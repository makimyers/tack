use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{bail, Context, Result};

pub type Version = [u64; 3];

/// Finds `cmd` on `$PATH` the way the OS would.
pub fn which(cmd: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(cmd))
        .find(|p| is_executable(p))
}

fn is_executable(p: &Path) -> bool {
    p.metadata()
        .map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

/// Runs one argv command (never through a shell) and returns its stdout.
pub fn run(argv: &[String]) -> Result<String> {
    let (prog, args) = argv.split_first().context("empty command")?;
    let out = Command::new(prog)
        .args(args)
        .stdin(Stdio::null())
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .with_context(|| format!("{prog}: not found"))?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        let last = stderr.lines().rev().find(|l| !l.trim().is_empty());
        bail!(
            "`{}` failed ({}){}",
            argv.join(" "),
            out.status,
            last.map(|l| format!(": {}", l.trim())).unwrap_or_default()
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

pub fn run_all(cmds: &[Vec<String>]) -> Result<()> {
    cmds.iter().try_for_each(|argv| run(argv).map(|_| ()))
}

/// The first `N.N.N` in `s`, e.g. `v22.19.0` or `pi 0.84.3 (abc)`.
pub fn version_in(s: &str) -> Option<Version> {
    let b = s.as_bytes();
    (0..b.len())
        .filter(|&i| b[i].is_ascii_digit() && (i == 0 || !b[i - 1].is_ascii_digit()))
        .find_map(|i| {
            let mut parts = s[i..].splitn(3, '.');
            let major = parts.next()?.parse().ok()?;
            let minor = parts.next()?.parse().ok()?;
            let patch: String = parts
                .next()?
                .chars()
                .take_while(|c| c.is_ascii_digit())
                .collect();
            Some([major, minor, patch.parse().ok()?])
        })
}

pub fn show(v: Version) -> String {
    format!("{}.{}.{}", v[0], v[1], v[2])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_and_compares_versions() {
        assert_eq!(version_in("v22.19.0\n"), Some([22, 19, 0]));
        assert_eq!(version_in("pi 0.84.3 (build 7)"), Some([0, 84, 3]));
        assert_eq!(version_in("rtk 1.2 then 3.4.5-beta"), Some([3, 4, 5]));
        assert_eq!(version_in("no version"), None);
        assert!(version_in("v22.9.0") < version_in("22.19.0"));
        assert!(version_in("0.84.10") > version_in("0.84.3"));
        assert!(version_in("1.0.0") > version_in("0.99.99"));
    }
}
