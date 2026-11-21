use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};

use crate::paths::Harness;

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct State {
    #[serde(default)]
    pub enabled: BTreeMap<String, BTreeMap<String, bool>>,
    /// Tools whose enable steps tack ran for a harness (and has not undone since).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub wired: BTreeMap<String, BTreeMap<String, bool>>,
}

/// The one command that gets past a broken state file.
pub fn fix(file: &Path) -> String {
    format!("mv {0} {0}.bak && tack install", file.display())
}

impl State {
    pub fn load(file: &Path) -> Result<State> {
        if !file.exists() {
            return Ok(State::default());
        }
        let text =
            std::fs::read_to_string(file).with_context(|| format!("reading {}", file.display()))?;
        toml::from_str(&text).map_err(|e| {
            let line = e
                .span()
                .map(|r| format!(" at line {}", text[..r.start].matches('\n').count() + 1))
                .unwrap_or_default();
            anyhow!(
                "{} is broken{line}: {}",
                file.display(),
                e.message().trim().replace('\n', "; ")
            )
        })
    }

    pub fn save(&self, file: &Path) -> Result<()> {
        if let Some(dir) = file.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(file, toml::to_string(self)?)
            .with_context(|| format!("writing {}", file.display()))
    }

    pub fn get(&self, name: &str, h: Harness) -> bool {
        self.enabled
            .get(name)
            .and_then(|m| m.get(h.name()))
            .copied()
            .unwrap_or(false)
    }

    pub fn set(&mut self, name: &str, h: Harness, on: bool) {
        self.enabled
            .entry(name.to_string())
            .or_default()
            .insert(h.name().to_string(), on);
    }

    pub fn wired(&self, name: &str, h: Harness) -> bool {
        self.wired
            .get(name)
            .is_some_and(|m| m.get(h.name()) == Some(&true))
    }

    pub fn set_wired(&mut self, name: &str, h: Harness, wired: bool) {
        if wired {
            self.wired
                .entry(name.to_string())
                .or_default()
                .insert(h.name().to_string(), true);
        } else if let Some(m) = self.wired.get_mut(name) {
            m.remove(h.name());
            if m.is_empty() {
                self.wired.remove(name);
            }
        }
    }

    /// Fills in the default for every (component, harness) not yet in state; never flips an existing one.
    pub fn add_default(&mut self, name: &str, harnesses: &[Harness], default: &[Harness]) {
        let entry = self.enabled.entry(name.to_string()).or_default();
        for h in harnesses {
            entry
                .entry(h.name().to_string())
                .or_insert(default.contains(h));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_never_flip_existing_entries() {
        let both = [Harness::Claude, Harness::Pi];
        let mut s = State::default();
        s.set("a", Harness::Claude, false);
        s.add_default("a", &both, &both);
        s.add_default("b", &both, &[Harness::Pi]);
        assert!(!s.get("a", Harness::Claude));
        assert!(s.get("a", Harness::Pi));
        assert!(!s.get("b", Harness::Claude));
        assert!(s.get("b", Harness::Pi));

        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("state.toml");
        s.save(&file).unwrap();
        let text = std::fs::read_to_string(&file).unwrap();
        assert!(text.contains("[enabled.a]"), "{text}");
        let back = State::load(&file).unwrap();
        assert_eq!(back.enabled, s.enabled);
    }

    #[test]
    fn wired_is_recorded_and_cleared() {
        let mut s = State::default();
        s.set_wired("rtk", Harness::Claude, true);
        s.set_wired("rtk", Harness::Pi, true);
        s.set_wired("rtk", Harness::Pi, false);
        assert!(s.wired("rtk", Harness::Claude) && !s.wired("rtk", Harness::Pi));
        let text = toml::to_string(&s).unwrap();
        assert!(text.contains("[wired.rtk]\nclaude = true"), "{text}");
        s.set_wired("rtk", Harness::Claude, false);
        assert!(!toml::to_string(&s).unwrap().contains("wired"));
    }
}
