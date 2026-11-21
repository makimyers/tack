//! The pi side of the guard: one global extension file that calls `tack guard --harness pi`.
use std::fs;

use anyhow::{bail, Context, Result};

use crate::paths::Paths;

const TEMPLATE: &str = include_str!("../assets/pi/tack-guard.ts");
const MARKER: &str = "// tack-guard: written by tack.";

pub fn install_extension(paths: &Paths) -> Result<bool> {
    let file = paths.pi_extension();
    let want = render(paths);
    match fs::read_to_string(&file) {
        Ok(have) if have == want => return Ok(false),
        Ok(have) if !have.contains(MARKER) => {
            bail!(
                "{} exists (not tack's); move it away, then retry",
                file.display()
            )
        }
        _ => {}
    }
    if let Some(dir) = file.parent() {
        fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    }
    fs::write(&file, want).with_context(|| format!("writing {}", file.display()))?;
    Ok(true)
}

pub fn remove_extension(paths: &Paths) -> Result<bool> {
    let file = paths.pi_extension();
    match fs::read_to_string(&file) {
        Ok(have) if have.contains(MARKER) => {
            fs::remove_file(&file).with_context(|| format!("removing {}", file.display()))?;
            Ok(true)
        }
        _ => Ok(false),
    }
}

pub fn extension_installed(paths: &Paths) -> bool {
    fs::read_to_string(paths.pi_extension()).is_ok_and(|have| have == render(paths))
}

fn render(paths: &Paths) -> String {
    // A JSON string is a valid TypeScript string literal, so any path is quoted safely.
    let bin = serde_json::to_string(&paths.tack_bin().to_string_lossy()).expect("string to JSON");
    TEMPLATE.replace("\"__TACK_BIN__\"", &bin)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_is_idempotent_and_removal_deletes_it() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::from_home(dir.path());
        assert!(!extension_installed(&paths));
        assert!(install_extension(&paths).unwrap());
        assert!(extension_installed(&paths));
        assert!(!install_extension(&paths).unwrap());

        let text = fs::read_to_string(paths.pi_extension()).unwrap();
        let bin = paths.tack_bin();
        assert!(text.contains(&format!("const TACK_BIN: string = \"{}\";", bin.display())));
        assert!(!text.contains("__TACK_BIN__"));

        assert!(remove_extension(&paths).unwrap());
        assert!(!paths.pi_extension().exists());
        assert!(!remove_extension(&paths).unwrap());
    }

    #[test]
    fn stale_copy_is_rewritten() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::from_home(dir.path());
        fs::create_dir_all(paths.pi_extension().parent().unwrap()).unwrap();
        fs::write(paths.pi_extension(), format!("{MARKER} old\n")).unwrap();
        assert!(!extension_installed(&paths));
        assert!(install_extension(&paths).unwrap());
        assert!(extension_installed(&paths));
    }

    #[test]
    fn a_file_that_is_not_tacks_is_left_alone() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::from_home(dir.path());
        fs::create_dir_all(paths.pi_extension().parent().unwrap()).unwrap();
        fs::write(paths.pi_extension(), "// mine\n").unwrap();
        let err = install_extension(&paths).unwrap_err().to_string();
        assert!(err.contains("not tack's"), "{err}");
        assert!(!remove_extension(&paths).unwrap());
        assert_eq!(
            fs::read_to_string(paths.pi_extension()).unwrap(),
            "// mine\n"
        );
    }
}
