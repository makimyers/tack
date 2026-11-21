use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

#[derive(Debug, PartialEq, Eq)]
pub enum Status {
    Missing,
    /// A symlink tack made: its target is inside `$TACK_HOME`.
    Ours(PathBuf),
    /// Anything else at that path; tack never touches it.
    Foreign,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Change {
    Done,
    Same,
    Foreign,
}

pub fn status(link: &Path, tack_home: &Path) -> Status {
    match fs::symlink_metadata(link) {
        Err(_) => Status::Missing,
        Ok(m) if m.file_type().is_symlink() => match fs::read_link(link) {
            Ok(target) if target.starts_with(tack_home) => Status::Ours(target),
            _ => Status::Foreign,
        },
        Ok(_) => Status::Foreign,
    }
}

pub fn ensure(link: &Path, target: &Path, tack_home: &Path) -> Result<Change> {
    match status(link, tack_home) {
        Status::Foreign => return Ok(Change::Foreign),
        Status::Ours(t) if t == target => return Ok(Change::Same),
        Status::Ours(_) => fs::remove_file(link)?,
        Status::Missing => {
            if let Some(dir) = link.parent() {
                fs::create_dir_all(dir)?;
            }
        }
    }
    symlink(target, link).with_context(|| format!("linking {}", link.display()))?;
    Ok(Change::Done)
}

pub fn remove(link: &Path, tack_home: &Path) -> Result<Change> {
    match status(link, tack_home) {
        Status::Ours(_) => {
            fs::remove_file(link).with_context(|| format!("removing {}", link.display()))?;
            Ok(Change::Done)
        }
        Status::Missing => Ok(Change::Same),
        Status::Foreign => Ok(Change::Foreign),
    }
}

/// Removes every tack link in `dir` whose name `keep` rejects; returns the removed paths.
pub fn prune(dir: &Path, tack_home: &Path, keep: impl Fn(&str) -> bool) -> Result<Vec<PathBuf>> {
    let mut removed = Vec::new();
    let Ok(entries) = fs::read_dir(dir) else {
        return Ok(removed);
    };
    for e in entries.flatten() {
        if !keep(&e.file_name().to_string_lossy()) && remove(&e.path(), tack_home)? == Change::Done
        {
            removed.push(e.path());
        }
    }
    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_links_into_tack_home_are_touched() {
        let tmp = tempfile::tempdir().unwrap();
        let tack = tmp.path().join(".tack");
        let a = tack.join("store/a");
        let b = tack.join("store/b");
        let skills = tmp.path().join("skills");

        let link = skills.join("x");
        assert_eq!(ensure(&link, &a, &tack).unwrap(), Change::Done);
        assert_eq!(ensure(&link, &a, &tack).unwrap(), Change::Same);
        assert_eq!(ensure(&link, &b, &tack).unwrap(), Change::Done);
        assert_eq!(fs::read_link(&link).unwrap(), b);
        assert_eq!(remove(&link, &tack).unwrap(), Change::Done);
        assert_eq!(remove(&link, &tack).unwrap(), Change::Same);

        let dir = skills.join("mine");
        fs::create_dir_all(&dir).unwrap();
        let elsewhere = skills.join("theirs");
        symlink(tmp.path().join("other"), &elsewhere).unwrap();
        for foreign in [&dir, &elsewhere] {
            assert_eq!(status(foreign, &tack), Status::Foreign);
            assert_eq!(ensure(foreign, &a, &tack).unwrap(), Change::Foreign);
            assert_eq!(remove(foreign, &tack).unwrap(), Change::Foreign);
        }
        assert!(dir.is_dir());
        assert_eq!(fs::read_link(&elsewhere).unwrap(), tmp.path().join("other"));
    }
}
