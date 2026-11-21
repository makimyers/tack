use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use include_dir::{include_dir, Dir};

use crate::manifest::Skill;
use crate::paths::Paths;
use crate::run;

pub static OWN: Dir<'static> = include_dir!("$CARGO_MANIFEST_DIR/skills");

const MARKER: &str = ".tack-complete";

/// Replaces `store/own/` with the embedded `skills/` tree.
pub fn write_own(paths: &Paths) -> Result<()> {
    let dir = paths.store().join("own");
    if dir.exists() {
        fs::remove_dir_all(&dir)?;
    }
    fs::create_dir_all(&dir)?;
    // shortcut: include_dir does not keep file modes, so own skills cannot ship executable scripts.
    OWN.extract(&dir)
        .with_context(|| format!("writing {}", dir.display()))
}

pub fn skill_dir(paths: &Paths, s: &Skill) -> PathBuf {
    let root = if s.is_own() {
        paths.store().join("own")
    } else {
        checkout_dir(paths, &s.source, &s.rev)
    };
    root.join(&s.path)
}

fn checkout_dir(paths: &Paths, url: &str, rev: &str) -> PathBuf {
    paths
        .store()
        .join(format!("{}-{}", slug(url), &rev[..12.min(rev.len())]))
}

fn slug(url: &str) -> String {
    let url = url.split_once("://").map_or(url, |(_, rest)| rest);
    let url = url.strip_suffix(".git").unwrap_or(url);
    let mut out = String::new();
    for c in url.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    // Long local paths would overflow the file-name limit; the tail is the distinctive part.
    let start = out.len().saturating_sub(60);
    out[start..].trim_matches('-').to_string()
}

/// One shallow checkout of `url` at `rev`; skipped when a complete one is already there.
fn fetch(url: &str, rev: &str, dir: &Path) -> Result<()> {
    if dir.join(MARKER).exists() {
        return Ok(());
    }
    if dir.exists() {
        fs::remove_dir_all(dir)?;
    }
    fs::create_dir_all(dir)?;
    let d = dir.to_string_lossy().to_string();
    let git = |args: &[&str]| {
        let mut argv = vec!["git".to_string(), "-C".to_string(), d.clone()];
        argv.extend(args.iter().map(|a| a.to_string()));
        run::run(&argv)
    };
    git(&["init", "-q"])?;
    git(&["fetch", "-q", "--depth", "1", url, rev])?;
    git(&["checkout", "-q", "FETCH_HEAD"])?;
    fs::write(dir.join(MARKER), "")?;
    Ok(())
}

/// Makes the checkout of every given git skill present. Prints one line per failed skill and returns
/// their names.
pub fn fetch_all<'a>(paths: &Paths, skills: impl Iterator<Item = &'a Skill>) -> BTreeSet<String> {
    let mut by_source: BTreeMap<(&str, &str), Vec<&Skill>> = BTreeMap::new();
    for s in skills.filter(|s| !s.is_own()) {
        by_source.entry((&s.source, &s.rev)).or_default().push(s);
    }
    let mut failed = BTreeSet::new();
    for ((url, rev), members) in by_source {
        let result = fetch(url, rev, &checkout_dir(paths, url, rev));
        for s in members {
            let err = match &result {
                Err(e) => Some(format!("{e:#}")),
                Ok(()) => check_skill(paths, s).err().map(|e| format!("{e:#}")),
            };
            if let Some(err) = err {
                println!("fail: {}: {err}", s.name);
                failed.insert(s.name.clone());
            }
        }
    }
    failed
}

/// Makes one skill's files present (used by `tack on`).
pub fn ensure_skill(paths: &Paths, s: &Skill) -> Result<()> {
    if check_skill(paths, s).is_ok() {
        return Ok(());
    }
    if s.is_own() {
        write_own(paths)?;
    } else {
        fetch(&s.source, &s.rev, &checkout_dir(paths, &s.source, &s.rev))?;
    }
    check_skill(paths, s)
}

fn check_skill(paths: &Paths, s: &Skill) -> Result<()> {
    let file = skill_dir(paths, s).join("SKILL.md");
    if !file.is_file() {
        bail!("{} is missing", file.display());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugs_are_path_safe() {
        assert_eq!(
            slug("https://github.com/obra/Superpowers.git"),
            "github-com-obra-superpowers"
        );
        assert_eq!(slug("/tmp/x/my repo"), "tmp-x-my-repo");
        let long = format!("/{}/skills-repo", "very-long-folder/".repeat(20));
        let s = slug(&long);
        assert!(s.len() <= 60 && s.ends_with("skills-repo"), "{s}");
    }

    #[test]
    fn own_skills_are_written_out() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::from_home(tmp.path());
        write_own(&paths).unwrap();
        write_own(&paths).unwrap();
        let skill = OWN
            .dirs()
            .map(|d| d.path().join("SKILL.md"))
            .find(|f| OWN.get_file(f).is_some())
            .expect("an own skill");
        assert!(paths.store().join("own").join(skill).is_file());
    }
}
