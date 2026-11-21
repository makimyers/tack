//! T3 Code provider defaults: Codex off, pi on, unless the owner already chose.
use anyhow::{anyhow, Result};
use serde_json::json;

use crate::claude::{read, write};
use crate::paths::Paths;

const DEFAULTS: [(&str, bool, &str); 2] = [("codex", false, "Codex off"), ("pi", true, "pi on")];

// T3 Code adds about 60 tools of its own to every pi session (about 16,000 prompt tokens measured
// on 0.1.11), too much for a local model. Both names are needed: the public one and its alias.
const PI_LAUNCH_ARGS: &str = "--exclude-tools mcp__t3-code__*,mcp__t3_code__*";

/// Adds each missing provider entry; returns what was changed, e.g. `["Codex off", "pi on"]`.
pub fn set_providers(paths: &Paths) -> Result<Vec<&'static str>> {
    let file = paths.t3_settings();
    let invalid = || {
        anyhow!(
            "{} is not valid T3 Code settings; left alone",
            file.display()
        )
    };
    let mut settings = match read(&file) {
        Ok(s) => s.unwrap_or_else(|| json!({})),
        Err(e) if e.is::<serde_json::Error>() => return Err(invalid()),
        Err(e) => return Err(e),
    };
    let providers = settings
        .as_object_mut()
        .ok_or_else(invalid)?
        .entry("providerInstances")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .ok_or_else(invalid)?;
    let mut changed = Vec::new();
    for (id, enabled, label) in DEFAULTS {
        let chosen = providers.contains_key(id) || providers.values().any(|v| v["driver"] == id);
        if !chosen {
            let mut entry = json!({"driver": id, "enabled": enabled});
            if id == "pi" {
                entry["config"] = json!({"launchArgs": PI_LAUNCH_ARGS});
            }
            providers.insert(id.into(), entry);
            changed.push(label);
        }
    }
    if !changed.is_empty() {
        write(&file, &settings)?;
    }
    Ok(changed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    use std::fs;

    fn setup() -> (tempfile::TempDir, Paths) {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::from_home(dir.path());
        (dir, paths)
    }

    fn settings(paths: &Paths) -> Value {
        serde_json::from_str(&fs::read_to_string(paths.t3_settings()).unwrap()).unwrap()
    }

    fn seed(paths: &Paths, text: &str) {
        fs::create_dir_all(paths.t3_settings().parent().unwrap()).unwrap();
        fs::write(paths.t3_settings(), text).unwrap();
    }

    #[test]
    fn creates_the_file_and_second_run_changes_nothing() {
        let (_d, paths) = setup();
        assert_eq!(set_providers(&paths).unwrap(), ["Codex off", "pi on"]);
        assert_eq!(
            settings(&paths),
            json!({"providerInstances": {
                "codex": {"driver": "codex", "enabled": false},
                "pi": {"driver": "pi", "enabled": true, "config": {"launchArgs": PI_LAUNCH_ARGS}}
            }})
        );
        assert!(PI_LAUNCH_ARGS.starts_with("--exclude-tools mcp__t3-code__*"));
        let first = fs::read(paths.t3_settings()).unwrap();
        assert!(set_providers(&paths).unwrap().is_empty());
        assert_eq!(fs::read(paths.t3_settings()).unwrap(), first);
    }

    #[test]
    fn keeps_other_keys_order_and_providers() {
        let (_d, paths) = setup();
        seed(
            &paths,
            r#"{"zeta": 1, "providerInstances": {"mine": {"driver": "x"}}, "alpha": 2}"#,
        );
        set_providers(&paths).unwrap();
        let s = settings(&paths);
        let keys: Vec<_> = s.as_object().unwrap().keys().cloned().collect();
        assert_eq!(keys, ["zeta", "providerInstances", "alpha"]);
        let ids: Vec<_> = s["providerInstances"]
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect();
        assert_eq!(ids, ["mine", "codex", "pi"]);
        assert_eq!(s["providerInstances"]["mine"], json!({"driver": "x"}));
    }

    #[test]
    fn existing_codex_entry_is_left_alone() {
        let (_d, paths) = setup();
        seed(
            &paths,
            r#"{"providerInstances": {"codex": {"driver": "codex", "enabled": true}}}"#,
        );
        assert_eq!(set_providers(&paths).unwrap(), ["pi on"]);
        let s = settings(&paths);
        assert_eq!(s["providerInstances"]["codex"]["enabled"], json!(true));
        assert_eq!(s["providerInstances"]["pi"]["enabled"], json!(true));
    }

    #[test]
    fn an_entry_with_the_same_driver_under_another_id_counts() {
        let (_d, paths) = setup();
        let text = r#"{"providerInstances": {"pi-local": {"driver": "pi"}, "codex-work": {"driver": "codex"}}}"#;
        seed(&paths, text);
        assert!(set_providers(&paths).unwrap().is_empty());
        assert_eq!(fs::read_to_string(paths.t3_settings()).unwrap(), text);
    }

    #[test]
    fn invalid_settings_are_left_byte_for_byte() {
        for text in ["{ not json", "[]", r#"{"providerInstances": []}"#] {
            let (_d, paths) = setup();
            seed(&paths, text);
            let err = set_providers(&paths).unwrap_err().to_string();
            assert!(
                err.ends_with("is not valid T3 Code settings; left alone"),
                "{err}"
            );
            assert_eq!(fs::read_to_string(paths.t3_settings()).unwrap(), text);
        }
    }
}
