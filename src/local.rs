//! The one local model: pi's default provider (written by `install`), started by `open`.
use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::{anyhow, Context, Result};
use serde_json::{json, Map, Value};

use crate::claude::{read, write};
use crate::manifest::LocalModel;
use crate::paths::{Harness, Paths};

pub const PROVIDER: &str = "tack-local";

/// Adds the `tack-local` provider to pi's models.json and makes it pi's default.
/// Returns true when either file changed. Other providers and settings stay.
pub fn wire_pi(paths: &Paths, lm: &LocalModel) -> Result<bool> {
    let dir = paths.config_dir(Harness::Pi);
    let provider = json!({
        "baseUrl": lm.base_url,
        "api": "openai-completions",
        "apiKey": "local",
        "compat": {"supportsDeveloperRole": false, "supportsReasoningEffort": false},
        "models": [{"id": lm.model, "name": lm.name, "contextWindow": lm.context, "maxTokens": 16384}]
    });
    let models = edit(paths, &dir.join("models.json"), |o| {
        let providers = o
            .entry("providers")
            .or_insert_with(|| json!({}))
            .as_object_mut()
            .ok_or_else(|| anyhow!("\"providers\" is not an object"))?;
        providers.insert(PROVIDER.into(), provider);
        Ok(())
    })?;
    let settings = edit(paths, &dir.join("settings.json"), |o| {
        o.insert("defaultProvider".into(), json!(PROVIDER));
        o.insert("defaultModel".into(), json!(lm.model));
        Ok(())
    })?;
    Ok(models || settings)
}

/// Runs the model's `up` command in the background, its output in `$TACK_HOME/local-model.log`,
/// so T3 Code opens while the weights load.
pub fn start(paths: &Paths, lm: &LocalModel) -> Result<()> {
    fs::create_dir_all(&paths.tack)?;
    let log_file = paths.tack.join("local-model.log");
    let log =
        fs::File::create(&log_file).with_context(|| format!("creating {}", log_file.display()))?;
    Command::new(&lm.up[0])
        .args(&lm.up[1..])
        .stdin(Stdio::null())
        .stdout(log.try_clone()?)
        .stderr(log)
        .spawn()
        .with_context(|| format!("{}: not found", lm.up[0]))?;
    Ok(())
}

fn edit(
    paths: &Paths,
    file: &Path,
    apply: impl FnOnce(&mut Map<String, Value>) -> Result<()>,
) -> Result<bool> {
    let mut value = read(file)?.unwrap_or_else(|| json!({}));
    let before = value.clone();
    let obj = value
        .as_object_mut()
        .ok_or_else(|| anyhow!("{} is not a JSON object; left alone", file.display()))?;
    apply(obj).with_context(|| format!("{}; left alone", file.display()))?;
    if value == before {
        return Ok(false);
    }
    backup(paths, file)?;
    write(file, &value)?;
    Ok(true)
}

/// Keeps the owner's file as it was before tack's first edit.
fn backup(paths: &Paths, file: &Path) -> Result<()> {
    let name = format!(
        "pi-{}",
        file.file_name().and_then(|n| n.to_str()).unwrap_or("file")
    );
    let dest = paths.backup_dir().join(name);
    if dest.exists() || !file.exists() {
        return Ok(());
    }
    fs::create_dir_all(paths.backup_dir())?;
    fs::copy(file, &dest).with_context(|| format!("backing up {}", file.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model() -> LocalModel {
        LocalModel {
            name: "Qwen fast".into(),
            up: vec!["/bin/true".into()],
            base_url: "http://127.0.0.1:8080/v1".into(),
            model: "qwen-fast".into(),
            context: 131072,
        }
    }

    fn json_at(p: &Path) -> Value {
        serde_json::from_str(&fs::read_to_string(p).unwrap()).unwrap()
    }

    #[test]
    fn wires_pi_keeps_other_entries_and_backs_up_once() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::from_home(dir.path());
        let pi = paths.config_dir(Harness::Pi).to_path_buf();
        fs::create_dir_all(&pi).unwrap();
        let mine = r#"{"providers": {"mine": {"baseUrl": "x"}}}"#;
        fs::write(pi.join("models.json"), mine).unwrap();
        fs::write(pi.join("settings.json"), r#"{"theme": "dark"}"#).unwrap();

        assert!(wire_pi(&paths, &model()).unwrap());
        let m = json_at(&pi.join("models.json"));
        assert_eq!(m["providers"]["mine"]["baseUrl"], "x");
        assert_eq!(m["providers"][PROVIDER]["models"][0]["id"], "qwen-fast");
        assert_eq!(
            m["providers"][PROVIDER]["models"][0]["contextWindow"],
            131072
        );
        let s = json_at(&pi.join("settings.json"));
        assert_eq!(s["theme"], "dark");
        assert_eq!(s["defaultProvider"], PROVIDER);
        assert_eq!(s["defaultModel"], "qwen-fast");
        let backup = paths.backup_dir().join("pi-models.json");
        assert_eq!(fs::read_to_string(&backup).unwrap(), mine);

        assert!(
            !wire_pi(&paths, &model()).unwrap(),
            "second run changes nothing"
        );
        assert_eq!(fs::read_to_string(&backup).unwrap(), mine);
    }

    #[test]
    fn creates_missing_files_and_refuses_a_non_object() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::from_home(dir.path());
        assert!(wire_pi(&paths, &model()).unwrap());
        let pi = paths.config_dir(Harness::Pi).to_path_buf();
        assert_eq!(
            json_at(&pi.join("settings.json"))["defaultModel"],
            "qwen-fast"
        );

        fs::write(pi.join("models.json"), "[]").unwrap();
        let err = wire_pi(&paths, &model()).unwrap_err().to_string();
        assert!(err.contains("not a JSON object; left alone"), "{err}");
        assert_eq!(fs::read_to_string(pi.join("models.json")).unwrap(), "[]");
    }

    #[test]
    fn start_logs_to_tack_home() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::from_home(dir.path());
        let mut lm = model();
        lm.up = vec!["/bin/sh".into(), "-c".into(), "echo started".into()];
        start(&paths, &lm).unwrap();
        let log = paths.tack.join("local-model.log");
        for _ in 0..50 {
            if fs::read_to_string(&log)
                .unwrap_or_default()
                .contains("started")
            {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        panic!("up command did not run");
    }
}
