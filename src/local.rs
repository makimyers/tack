//! The local model provider and Tack's T3 Code script buttons.
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::path::Path;
use std::process::Command;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use serde_json::{json, Map, Value};

use crate::claude::{read, write};
use crate::manifest::LocalModel;
use crate::paths::{Harness, Paths};

pub const PROVIDER: &str = "tack-local";
const SCRIPT_PREFIX: &str = "tack-model-";
const SKILLS_SCRIPT_ID: &str = "tack-skills-tools";

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
    let models = edit(paths, &dir.join("models.json"), "pi-models.json", |o| {
        let providers = o
            .entry("providers")
            .or_insert_with(|| json!({}))
            .as_object_mut()
            .ok_or_else(|| anyhow!("\"providers\" is not an object"))?;
        providers.insert(PROVIDER.into(), provider);
        Ok(())
    })?;
    let settings = edit(paths, &dir.join("settings.json"), "pi-settings.json", |o| {
        o.insert("defaultProvider".into(), json!(PROVIDER));
        o.insert("defaultModel".into(), json!(lm.model));
        Ok(())
    })?;
    Ok(models || settings)
}

/// Updates Tack's entries in T3 Code's scripts menu.
pub fn wire_t3(paths: &Paths, has_local_model: bool) -> Result<bool> {
    let tack = paths
        .tack_bin()
        .display()
        .to_string()
        .replace('\'', "'\\''");
    let mut ours = Vec::new();
    if has_local_model {
        ours.extend(
            [("status", "test"), ("on", "play"), ("off", "debug")].map(|(verb, icon)| {
                json!({
                    "id": format!("{SCRIPT_PREFIX}{verb}"),
                    "name": format!("Local model: {verb}"),
                    "command": format!("'{tack}' model {verb}"),
                    "icon": icon,
                    "runOnWorktreeCreate": false
                })
            }),
        );
    }
    ours.push(json!({
        "id": SKILLS_SCRIPT_ID,
        "name": "Skills & tools",
        "command": format!("'{tack}' list"),
        "icon": "test",
        "runOnWorktreeCreate": false
    }));
    edit(paths, &paths.t3_settings(), "t3-settings.json", |o| {
        let scripts = o
            .entry("defaultProjectScripts")
            .or_insert_with(|| json!([]))
            .as_array_mut()
            .ok_or_else(|| anyhow!("\"defaultProjectScripts\" is not a list"))?;
        scripts.retain(|s| {
            !s["id"]
                .as_str()
                .is_some_and(|id| id == SKILLS_SCRIPT_ID || id.starts_with(SCRIPT_PREFIX))
        });
        scripts.extend(ours);
        Ok(())
    })
}

/// `Some(true)` when the server lists the model, `Some(false)` when nothing answers or it serves
/// another model, `None` when the URL cannot be probed (https).
pub fn running(lm: &LocalModel) -> Option<bool> {
    let rest = lm.base_url.strip_prefix("http://")?;
    let (hostport, path) = rest.split_once('/').unwrap_or((rest, ""));
    let hostport = if hostport.contains(':') {
        hostport.to_string()
    } else {
        format!("{hostport}:80")
    };
    let probe = || -> std::io::Result<String> {
        let addr = hostport
            .to_socket_addrs()?
            .next()
            .ok_or(std::io::ErrorKind::NotFound)?;
        let mut s = TcpStream::connect_timeout(&addr, Duration::from_secs(1))?;
        s.set_read_timeout(Some(Duration::from_secs(3)))?;
        let path = path.trim_end_matches('/');
        let req =
            format!("GET /{path}/models HTTP/1.0\r\nHost: {hostport}\r\nConnection: close\r\n\r\n");
        s.write_all(req.as_bytes())?;
        let mut body = String::new();
        s.read_to_string(&mut body)?;
        Ok(body)
    };
    Some(probe().is_ok_and(|b| b.contains(&format!("\"{}\"", lm.model))))
}

/// Runs one of the model's commands in this terminal, so its progress shows.
pub fn run(argv: &[String]) -> Result<bool> {
    let status = Command::new(&argv[0])
        .args(&argv[1..])
        .status()
        .with_context(|| format!("{}: not found", argv[0]))?;
    Ok(status.success())
}

fn edit(
    paths: &Paths,
    file: &Path,
    backup_name: &str,
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
    backup(paths, file, backup_name)?;
    write(file, &value)?;
    Ok(true)
}

/// Keeps the owner's file as it was before tack's first edit.
fn backup(paths: &Paths, file: &Path, name: &str) -> Result<()> {
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
    use std::net::TcpListener;

    fn model() -> LocalModel {
        LocalModel {
            name: "Qwen fast".into(),
            up: vec!["/bin/true".into()],
            down: vec![],
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
    fn t3_buttons_are_added_once_and_the_owners_scripts_stay() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::from_home(dir.path());
        fs::create_dir_all(paths.t3_settings().parent().unwrap()).unwrap();
        let mine = r#"{"defaultProjectScripts": [{"id": "dev", "name": "Dev", "command": "npm run dev", "icon": "play", "runOnWorktreeCreate": false}]}"#;
        fs::write(paths.t3_settings(), mine).unwrap();

        assert!(wire_t3(&paths, true).unwrap());
        let s = json_at(&paths.t3_settings());
        let ids: Vec<_> = s["defaultProjectScripts"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v["id"].as_str().unwrap().to_string())
            .collect();
        assert_eq!(
            ids,
            [
                "dev",
                "tack-model-status",
                "tack-model-on",
                "tack-model-off",
                "tack-skills-tools"
            ]
        );
        let on = &s["defaultProjectScripts"][2];
        assert_eq!(on["name"], "Local model: on");
        assert!(on["command"]
            .as_str()
            .unwrap()
            .ends_with("/bin/tack' model on"));
        let skills = &s["defaultProjectScripts"][4];
        assert_eq!(skills["name"], "Skills & tools");
        assert!(skills["command"]
            .as_str()
            .unwrap()
            .ends_with("/bin/tack' list"));
        assert!(
            !wire_t3(&paths, true).unwrap(),
            "second run changes nothing"
        );
        assert!(wire_t3(&paths, false).unwrap());
        assert_eq!(
            json_at(&paths.t3_settings())["defaultProjectScripts"][1],
            *skills
        );
        assert_eq!(
            fs::read_to_string(paths.backup_dir().join("t3-settings.json")).unwrap(),
            mine
        );
    }

    #[test]
    fn running_needs_the_model_id_in_the_answer() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let serve = std::thread::spawn(move || {
            for body in [
                r#"{"data":[{"id":"qwen-fast"}]}"#,
                r#"{"data":[{"id":"other"}]}"#,
            ] {
                let (mut s, _) = listener.accept().unwrap();
                let mut buf = [0u8; 512];
                let n = s.read(&mut buf).unwrap();
                assert!(String::from_utf8_lossy(&buf[..n]).starts_with("GET /v1/models "));
                write!(s, "HTTP/1.0 200 OK\r\n\r\n{body}").unwrap();
            }
        });
        let mut lm = model();
        lm.base_url = format!("http://127.0.0.1:{port}/v1");
        assert_eq!(running(&lm), Some(true));
        assert_eq!(running(&lm), Some(false), "another model is not ours");
        serve.join().unwrap();
        assert_eq!(running(&lm), Some(false), "nothing listening");
        lm.base_url = "https://example.com/v1".into();
        assert_eq!(running(&lm), None);
    }
}
