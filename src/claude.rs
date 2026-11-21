//! The Claude Code side of the guard: one PreToolUse hook entry in `settings.json`.
use std::fs;
use std::path::Path;

use anyhow::{bail, Context, Result};
use serde_json::{json, Map, Value};

use crate::paths::Paths;

pub fn install_hook(paths: &Paths) -> Result<bool> {
    let file = paths.claude_settings();
    let command = hook_command(paths);
    let Some(mut settings) = read(&file)? else {
        let settings = json!({"hooks": {"PreToolUse": [entry(&command)]}});
        write(&file, &settings)?;
        return Ok(true);
    };
    if commands(&settings).any(|c| c == command) {
        return Ok(false);
    }
    backup(paths, &file)?;
    strip_tack_entries(&mut settings, &file)?;
    let hooks = settings
        .as_object_mut()
        .with_context(|| format!("{} is not a JSON object", file.display()))?
        .entry("hooks")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .with_context(|| format!("{}: `hooks` is not an object", file.display()))?;
    hooks
        .entry("PreToolUse")
        .or_insert_with(|| json!([]))
        .as_array_mut()
        .with_context(|| format!("{}: `hooks.PreToolUse` is not a list", file.display()))?
        .push(entry(&command));
    write(&file, &settings)?;
    Ok(true)
}

pub fn remove_hook(paths: &Paths) -> Result<bool> {
    let file = paths.claude_settings();
    let Some(mut settings) = read(&file)? else {
        return Ok(false);
    };
    if !strip_tack_entries(&mut settings, &file)? {
        return Ok(false);
    }
    // What existed before tack's first edit; no backup means tack created the file.
    let original = read(&paths.backup_dir().join("claude-settings.json"))
        .ok()
        .flatten();
    let had = |ptr: &str| original.as_ref().is_some_and(|o| o.pointer(ptr).is_some());
    if let Some(obj) = settings.as_object_mut() {
        let pre_empty = obj
            .get("hooks")
            .and_then(|h| h.get("PreToolUse"))
            .and_then(Value::as_array)
            .is_some_and(Vec::is_empty);
        if pre_empty && !had("/hooks/PreToolUse") {
            if let Some(h) = obj.get_mut("hooks").and_then(Value::as_object_mut) {
                h.remove("PreToolUse");
            }
        }
        let hooks_empty = obj
            .get("hooks")
            .and_then(Value::as_object)
            .is_some_and(Map::is_empty);
        if hooks_empty && !had("/hooks") {
            obj.remove("hooks");
        }
        if obj.is_empty() && original.is_none() {
            fs::remove_file(&file).with_context(|| format!("removing {}", file.display()))?;
            return Ok(true);
        }
    }
    write(&file, &settings)?;
    Ok(true)
}

pub fn hook_installed(paths: &Paths) -> bool {
    let command = hook_command(paths);
    matches!(read(&paths.claude_settings()), Ok(Some(s)) if commands(&s).any(|c| c == command))
}

fn hook_command(paths: &Paths) -> String {
    let bin = paths.tack_bin().to_string_lossy().into_owned();
    let plain = bin
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || "/._-+@".contains(c));
    let bin = if plain {
        bin
    } else {
        format!("'{}'", bin.replace('\'', r"'\''"))
    };
    // Claude Code blocks only on exit 2; hook commands run through bash, so any other failure
    // (missing binary, crash) is turned into a block here.
    format!("{bin} guard --harness claude || exit 2")
}

/// The ownership test from the spec; also matches a quoted binary path (`.../tack' guard`).
fn is_tack(command: &str) -> bool {
    command.contains("tack guard") || command.contains("tack' guard")
}

fn entry(command: &str) -> Value {
    json!({"matcher": "*", "hooks": [{"type": "command", "command": command}]})
}

fn commands(settings: &Value) -> impl Iterator<Item = &str> {
    settings
        .pointer("/hooks/PreToolUse")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|g| g.get("hooks")?.as_array())
        .flatten()
        .filter_map(|h| h.get("command")?.as_str())
}

/// Removes every PreToolUse hook whose command is tack's, and any group left empty by that.
/// Returns whether anything was removed.
fn strip_tack_entries(settings: &mut Value, file: &Path) -> Result<bool> {
    let Some(groups) = settings.pointer_mut("/hooks/PreToolUse") else {
        return Ok(false);
    };
    let Some(groups) = groups.as_array_mut() else {
        bail!("{}: `hooks.PreToolUse` is not a list", file.display());
    };
    let mut removed = false;
    groups.retain_mut(|g| {
        let Some(hooks) = g.get_mut("hooks").and_then(Value::as_array_mut) else {
            return true;
        };
        let before = hooks.len();
        hooks.retain(|h| {
            !h.get("command")
                .and_then(Value::as_str)
                .is_some_and(is_tack)
        });
        if hooks.len() == before {
            return true;
        }
        removed = true;
        !hooks.is_empty()
    });
    Ok(removed)
}

/// `Ok(None)` when the file does not exist; an error naming the file when it is not valid JSON.
pub(crate) fn read(file: &Path) -> Result<Option<Value>> {
    let text = match fs::read_to_string(file) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e).with_context(|| format!("reading {}", file.display())),
    };
    serde_json::from_str(&text).map(Some).with_context(|| {
        format!(
            "{} is not valid JSON; fix or move it, then retry",
            file.display()
        )
    })
}

pub(crate) fn write(file: &Path, settings: &Value) -> Result<()> {
    write_text(file, &(serde_json::to_string_pretty(settings)? + "\n"))
}

pub(crate) fn write_text(file: &Path, text: &str) -> Result<()> {
    if let Some(dir) = file.parent() {
        fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    }
    // Write next to the real file (through a dotfiles symlink) and rename, so a crash never
    // leaves the agent half a file.
    let target = fs::canonicalize(file).unwrap_or_else(|_| file.to_path_buf());
    let mut tmp = target.clone().into_os_string();
    tmp.push(".tack-tmp");
    let tmp = std::path::PathBuf::from(tmp);
    fs::write(&tmp, text).with_context(|| format!("writing {}", tmp.display()))?;
    let replaced = fs::metadata(&target)
        .map_or(Ok(()), |m| fs::set_permissions(&tmp, m.permissions()))
        .and_then(|()| fs::rename(&tmp, &target));
    if replaced.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    replaced.with_context(|| format!("replacing {}", target.display()))
}

fn backup(paths: &Paths, file: &Path) -> Result<()> {
    let dest = paths.backup_dir().join("claude-settings.json");
    if dest.exists() {
        return Ok(());
    }
    fs::create_dir_all(paths.backup_dir())
        .with_context(|| format!("creating {}", paths.backup_dir().display()))?;
    fs::copy(file, &dest).with_context(|| format!("backing up {}", file.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> (tempfile::TempDir, Paths) {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::from_home(dir.path());
        (dir, paths)
    }

    fn settings(paths: &Paths) -> Value {
        read(&paths.claude_settings()).unwrap().unwrap()
    }

    #[test]
    fn fresh_install_creates_settings_and_removal_deletes_it() {
        let (_d, paths) = setup();
        assert!(!hook_installed(&paths));
        assert!(install_hook(&paths).unwrap());
        assert!(hook_installed(&paths));
        let cmd = format!(
            "{} guard --harness claude || exit 2",
            paths.tack_bin().display()
        );
        assert_eq!(
            settings(&paths),
            json!({"hooks": {"PreToolUse": [
                {"matcher": "*", "hooks": [{"type": "command", "command": cmd}]}
            ]}})
        );
        assert!(remove_hook(&paths).unwrap());
        assert!(!paths.claude_settings().exists());
        assert!(!remove_hook(&paths).unwrap());
    }

    #[test]
    fn existing_settings_keep_other_hooks_keys_and_order() {
        let (_d, paths) = setup();
        let original = r#"{
  "zeta": 1,
  "hooks": {
    "PreToolUse": [
      { "matcher": "Bash", "hooks": [{ "type": "command", "command": "other-guard" }] }
    ],
    "Stop": []
  },
  "alpha": { "b": 2, "a": 1 }
}
"#;
        fs::create_dir_all(&paths.claude).unwrap();
        fs::write(paths.claude_settings(), original).unwrap();

        assert!(install_hook(&paths).unwrap());
        let after = fs::read_to_string(paths.claude_settings()).unwrap();
        let keys: Vec<_> = settings(&paths)
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect();
        assert_eq!(keys, ["zeta", "hooks", "alpha"]);
        assert!(after.find("\"b\"").unwrap() < after.find("\"a\": 1").unwrap());
        assert_eq!(commands(&settings(&paths)).count(), 2);
        assert_eq!(
            fs::read_to_string(paths.backup_dir().join("claude-settings.json")).unwrap(),
            original
        );

        assert!(remove_hook(&paths).unwrap());
        let restored: Value = serde_json::from_str(original).unwrap();
        assert_eq!(settings(&paths), restored);
    }

    #[test]
    fn second_install_is_byte_identical() {
        let (_d, paths) = setup();
        fs::create_dir_all(&paths.claude).unwrap();
        fs::write(paths.claude_settings(), r#"{"model": "x"}"#).unwrap();
        assert!(install_hook(&paths).unwrap());
        let first = fs::read(paths.claude_settings()).unwrap();
        assert!(!install_hook(&paths).unwrap());
        assert_eq!(fs::read(paths.claude_settings()).unwrap(), first);
    }

    #[test]
    fn removal_drops_only_what_tack_created() {
        let (_d, paths) = setup();
        fs::create_dir_all(&paths.claude).unwrap();
        fs::write(paths.claude_settings(), r#"{"model": "x"}"#).unwrap();
        install_hook(&paths).unwrap();
        assert!(remove_hook(&paths).unwrap());
        assert_eq!(settings(&paths), json!({"model": "x"}));

        let (_d, paths) = setup();
        fs::create_dir_all(&paths.claude).unwrap();
        fs::write(paths.claude_settings(), r#"{"hooks": {"PreToolUse": []}}"#).unwrap();
        install_hook(&paths).unwrap();
        assert!(remove_hook(&paths).unwrap());
        assert_eq!(settings(&paths), json!({"hooks": {"PreToolUse": []}}));
    }

    #[test]
    fn stale_tack_entry_is_replaced_and_group_with_other_hooks_kept() {
        let (_d, paths) = setup();
        fs::create_dir_all(&paths.claude).unwrap();
        let old = json!({"hooks": {"PreToolUse": [{"matcher": "*", "hooks": [
            {"type": "command", "command": "/old/.tack/bin/tack guard --harness claude"},
            {"type": "command", "command": "keep-me"}
        ]}]}});
        fs::write(paths.claude_settings(), old.to_string()).unwrap();
        assert!(install_hook(&paths).unwrap());
        let cmds: Vec<String> = commands(&settings(&paths)).map(String::from).collect();
        assert_eq!(cmds.len(), 2);
        assert_eq!(cmds[0], "keep-me");
        assert!(hook_installed(&paths));
        assert!(remove_hook(&paths).unwrap());
        assert_eq!(commands(&settings(&paths)).collect::<Vec<_>>(), ["keep-me"]);
    }

    #[test]
    fn old_form_entry_is_upgraded_not_duplicated() {
        let (_d, paths) = setup();
        fs::create_dir_all(&paths.claude).unwrap();
        let old = format!("{} guard --harness claude", paths.tack_bin().display());
        fs::write(
            paths.claude_settings(),
            json!({"hooks": {"PreToolUse": [entry(&old)]}}).to_string(),
        )
        .unwrap();
        assert!(!hook_installed(&paths));
        assert!(install_hook(&paths).unwrap());
        let cmds: Vec<String> = commands(&settings(&paths)).map(String::from).collect();
        assert_eq!(cmds, [format!("{old} || exit 2")]);
        assert!(hook_installed(&paths));
        assert!(!install_hook(&paths).unwrap());
        assert!(remove_hook(&paths).unwrap());
        assert_eq!(commands(&settings(&paths)).count(), 0);
    }

    #[test]
    fn hook_fails_closed_when_tack_cannot_run() {
        let (_d, paths) = setup();
        let out = std::process::Command::new("bash")
            .args(["-c", &hook_command(&paths)])
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(2));
    }

    #[test]
    fn write_replaces_in_one_step_and_keeps_a_symlink() {
        let (d, paths) = setup();
        fs::create_dir_all(&paths.claude).unwrap();
        let real = d.path().join("dotfiles-settings.json");
        fs::write(&real, r#"{"model": "x"}"#).unwrap();
        std::os::unix::fs::symlink(&real, paths.claude_settings()).unwrap();
        assert!(install_hook(&paths).unwrap());
        assert!(fs::symlink_metadata(paths.claude_settings())
            .unwrap()
            .file_type()
            .is_symlink());
        assert!(hook_installed(&paths));
        let leftovers: Vec<_> = fs::read_dir(d.path())
            .unwrap()
            .chain(fs::read_dir(&paths.claude).unwrap())
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|n| n.contains("tmp"))
            .collect();
        assert!(leftovers.is_empty(), "{leftovers:?}");
    }

    #[test]
    fn write_keeps_the_file_mode() {
        use std::os::unix::fs::PermissionsExt;
        let (_d, paths) = setup();
        fs::create_dir_all(&paths.claude).unwrap();
        let file = paths.claude_settings();
        fs::write(&file, r#"{"model": "x"}"#).unwrap();
        fs::set_permissions(&file, fs::Permissions::from_mode(0o600)).unwrap();
        assert!(install_hook(&paths).unwrap());
        let mode = fs::metadata(&file).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }

    #[test]
    fn invalid_json_is_an_error_and_never_overwritten() {
        let (_d, paths) = setup();
        fs::create_dir_all(&paths.claude).unwrap();
        fs::write(paths.claude_settings(), "{ not json").unwrap();
        let err = install_hook(&paths).unwrap_err().to_string();
        assert!(err.contains("settings.json"), "{err}");
        assert!(remove_hook(&paths).is_err());
        assert!(!hook_installed(&paths));
        assert_eq!(
            fs::read_to_string(paths.claude_settings()).unwrap(),
            "{ not json"
        );
    }

    #[test]
    fn path_with_spaces_is_quoted_and_still_owned() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::from_home(&dir.path().join("my home"));
        let cmd = hook_command(&paths);
        assert!(cmd.starts_with('\'') && is_tack(&cmd), "{cmd}");
        assert!(install_hook(&paths).unwrap());
        assert!(remove_hook(&paths).unwrap());
        assert!(!paths.claude_settings().exists());
    }
}
