//! The `rules` component: one managed block holding `assets/rules.md` in the file each agent
//! loads every session (Claude Code's CLAUDE.md, pi's AGENTS.md).
use std::fs;
use std::path::Path;

use anyhow::{bail, Context, Result};

use crate::claude;
use crate::doctor::tilde;
use crate::paths::{Harness, Paths};

const RULES: &str = include_str!("../assets/rules.md");
const START: &str = "<!-- tack:rules:start -->";
const END: &str = "<!-- tack:rules:end -->";
const EXTENSION: &str = include_str!("../assets/pi/tack-rules.ts");
const EXT_MARKER: &str = "// tack-rules: written by tack.";

#[derive(Debug, PartialEq)]
enum Edit {
    Changed,
    Same,
    Damaged,
}

fn block() -> String {
    format!("{START}\n{RULES}{END}\n")
}

/// Byte range from the start marker through the end marker's line; `Err` when the markers do not
/// form exactly one block.
fn find(text: &str) -> Result<Option<(usize, usize)>, ()> {
    let starts: Vec<usize> = text.match_indices(START).map(|(i, _)| i).collect();
    let ends: Vec<usize> = text.match_indices(END).map(|(i, _)| i).collect();
    match (starts.as_slice(), ends.as_slice()) {
        ([], []) => Ok(None),
        ([s], [e]) if s < e => {
            let mut e = e + END.len();
            if text[e..].starts_with('\n') {
                e += 1;
            }
            Ok(Some((*s, e)))
        }
        _ => Err(()),
    }
}

fn apply(file: &Path, on: bool) -> Result<Edit> {
    let have = match fs::read_to_string(file) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e).with_context(|| format!("reading {}", file.display())),
    };
    let Ok(found) = find(&have) else {
        return Ok(Edit::Damaged);
    };
    let want = match (found, on) {
        (Some((s, e)), true) => format!("{}{}{}", &have[..s], block(), &have[e..]),
        (None, true) if have.is_empty() => block(),
        (None, true) => {
            let nl = if have.ends_with('\n') { "" } else { "\n" };
            format!("{have}{nl}\n{}", block())
        }
        (Some((s, e)), false) => {
            let before = &have[..s];
            let before = before
                .strip_suffix('\n')
                .filter(|b| b.ends_with('\n'))
                .unwrap_or(before);
            format!("{before}{}", &have[e..])
        }
        (None, false) => return Ok(Edit::Same),
    };
    if want == have {
        return Ok(Edit::Same);
    }
    if !on && want.trim().is_empty() {
        fs::remove_file(file).with_context(|| format!("removing {}", file.display()))?;
    } else {
        claude::write_text(file, &want)?;
    }
    Ok(Edit::Changed)
}

/// Writes (`on`) or removes the block for `h`, and for pi the reminder extension. A damaged block
/// is reported and left alone.
pub fn set(paths: &Paths, h: Harness, on: bool) -> Result<bool> {
    let file = paths.rules_file(h);
    let block_changed = match apply(&file, on)? {
        Edit::Damaged => {
            println!(
                "warn: {} has a damaged tack rules block; left alone",
                tilde(&paths.home, &file)
            );
            false
        }
        e => e == Edit::Changed,
    };
    let ext_changed = h == Harness::Pi && set_extension(&paths.pi_rules_extension(), on)?;
    Ok(block_changed || ext_changed)
}

fn set_extension(file: &Path, on: bool) -> Result<bool> {
    match fs::read_to_string(file) {
        Ok(have) if on && have == EXTENSION => Ok(false),
        Ok(have) if !have.contains(EXT_MARKER) => {
            if on {
                bail!(
                    "{} exists (not tack's); move it away, then retry",
                    file.display()
                );
            }
            Ok(false)
        }
        Ok(_) if !on => {
            fs::remove_file(file).with_context(|| format!("removing {}", file.display()))?;
            Ok(true)
        }
        Err(_) if !on => Ok(false),
        _ => {
            claude::write_text(file, EXTENSION)?;
            Ok(true)
        }
    }
}

/// pi's reminder extension is there and matches `assets/pi/tack-rules.ts`.
pub fn extension_installed(paths: &Paths) -> bool {
    fs::read_to_string(paths.pi_rules_extension()).is_ok_and(|have| have == EXTENSION)
}

/// The block is there and matches `assets/rules.md`.
pub fn installed(paths: &Paths, h: Harness) -> bool {
    let Ok(text) = fs::read_to_string(paths.rules_file(h)) else {
        return false;
    };
    matches!(find(&text), Ok(Some((s, e))) if text[s..e] == block())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup(text: Option<&str>) -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("sub").join("CLAUDE.md");
        if let Some(t) = text {
            fs::create_dir_all(file.parent().unwrap()).unwrap();
            fs::write(&file, t).unwrap();
        }
        (dir, file)
    }

    fn read(file: &Path) -> String {
        fs::read_to_string(file).unwrap()
    }

    #[test]
    fn appends_after_a_blank_line_and_keeps_other_text() {
        let mine = "# Mine\n\nkeep this\n";
        let (_d, file) = setup(Some(mine));
        assert_eq!(apply(&file, true).unwrap(), Edit::Changed);
        assert_eq!(read(&file), format!("{mine}\n{}", block()));
        assert_eq!(apply(&file, true).unwrap(), Edit::Same);
        assert_eq!(apply(&file, false).unwrap(), Edit::Changed);
        assert_eq!(read(&file), mine);
    }

    #[test]
    fn creates_a_missing_file_and_off_deletes_it() {
        let (_d, file) = setup(None);
        assert_eq!(apply(&file, true).unwrap(), Edit::Changed);
        assert_eq!(read(&file), block());
        assert_eq!(apply(&file, false).unwrap(), Edit::Changed);
        assert!(!file.exists());
        assert_eq!(apply(&file, false).unwrap(), Edit::Same);
    }

    #[test]
    fn replaces_an_older_block_in_place() {
        let old = format!("top\n\n{START}\nold rules\n{END}\nbottom\n");
        let (_d, file) = setup(Some(&old));
        assert_eq!(apply(&file, true).unwrap(), Edit::Changed);
        assert_eq!(read(&file), format!("top\n\n{}bottom\n", block()));
        assert_eq!(apply(&file, false).unwrap(), Edit::Changed);
        assert_eq!(read(&file), "top\nbottom\n");
    }

    #[test]
    fn damaged_markers_are_left_byte_for_byte() {
        for text in [
            format!("a\n{START}\nx\n"),
            format!("a\n{END}\n"),
            format!("{END}\n{START}\n"),
            format!("{}{}", block(), block()),
        ] {
            let (_d, file) = setup(Some(&text));
            assert_eq!(apply(&file, true).unwrap(), Edit::Damaged);
            assert_eq!(apply(&file, false).unwrap(), Edit::Damaged);
            assert_eq!(read(&file), text);
        }
    }

    #[test]
    fn keeps_file_permissions() {
        use std::os::unix::fs::PermissionsExt;
        let (_d, file) = setup(Some("mine\n"));
        fs::set_permissions(&file, fs::Permissions::from_mode(0o600)).unwrap();
        apply(&file, true).unwrap();
        let mode = fs::metadata(&file).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }

    #[test]
    fn installed_checks_the_current_text() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::from_home(dir.path());
        assert!(!installed(&paths, Harness::Pi));
        assert!(set(&paths, Harness::Pi, true).unwrap());
        assert!(installed(&paths, Harness::Pi));
        assert!(!installed(&paths, Harness::Claude));
        assert!(paths.pi.join("AGENTS.md").is_file());
    }

    #[test]
    fn pi_extension_is_written_once_and_removed() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::from_home(dir.path());
        let ext = paths.pi_rules_extension();
        assert!(set(&paths, Harness::Pi, true).unwrap());
        assert_eq!(read(&ext), EXTENSION);
        assert!(extension_installed(&paths));
        assert!(!set(&paths, Harness::Pi, true).unwrap());
        assert!(set(&paths, Harness::Pi, false).unwrap());
        assert!(!ext.exists());
        assert!(!set(&paths, Harness::Pi, false).unwrap());

        assert!(set(&paths, Harness::Claude, true).unwrap());
        assert!(!ext.exists());

        fs::create_dir_all(ext.parent().unwrap()).unwrap();
        fs::write(&ext, "// mine\n").unwrap();
        assert!(set(&paths, Harness::Pi, true).is_err());
        set(&paths, Harness::Pi, false).unwrap();
        assert_eq!(read(&ext), "// mine\n");
    }
}
