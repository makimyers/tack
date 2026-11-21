use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component as PathPart, Path};

use anyhow::{bail, Context, Result};
use serde::Deserialize;

use crate::paths::{Harness, Paths};
use crate::run;
use crate::store;

const SKILLS: &str = include_str!("../manifest/skills.toml");
const TOOLS: &str = include_str!("../manifest/tools.toml");

pub type Steps = BTreeMap<String, Vec<Vec<String>>>;

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Skill {
    pub name: String,
    pub description: String,
    pub source: String,
    pub path: String,
    #[serde(default)]
    pub rev: String,
    pub harnesses: Vec<String>,
    pub default: Vec<String>,
    #[serde(default)]
    pub group: String,
}

impl Skill {
    pub fn is_own(&self) -> bool {
        self.source == "own"
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tool {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub harnesses: Vec<String>,
    #[serde(default)]
    pub default: Vec<String>,
    #[serde(default)]
    pub base: bool,
    #[serde(default)]
    pub required: bool,
    pub check: Vec<String>,
    #[serde(default)]
    pub min_version: String,
    #[serde(default)]
    pub hint: String,
    /// One line, a command or URL, to install the tool by hand when no install step matches.
    #[serde(default)]
    pub manual: String,
    #[serde(default)]
    pub install: Vec<InstallStep>,
    #[serde(default)]
    pub enable: Steps,
    #[serde(default)]
    pub disable: Steps,
    /// Per harness: a file, relative to that harness's config folder, that the enable steps create.
    #[serde(default)]
    pub marker: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InstallStep {
    pub os: String,
    #[serde(default)]
    pub needs: Vec<String>,
    pub run: Vec<Vec<String>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    #[serde(default)]
    pub skill: Vec<Skill>,
    #[serde(default)]
    pub tool: Vec<Tool>,
    /// The one local model pi uses; set in `$TACK_HOME/tack.toml`, never in the shipped catalogue.
    #[serde(default)]
    pub local_model: Option<LocalModel>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LocalModel {
    /// Shown in pi's model picker and in tack's output.
    pub name: String,
    /// argv that starts the server and returns; safe to run when it is already up.
    pub up: Vec<String>,
    /// argv that stops the server; empty = `tack model off` says it cannot.
    #[serde(default)]
    pub down: Vec<String>,
    /// OpenAI-compatible base URL, e.g. `http://127.0.0.1:8080/v1`.
    pub base_url: String,
    /// The id the server answers with on `/v1/models`.
    pub model: String,
    /// Context window the server is started with.
    pub context: u64,
}

/// Embedded catalogue (or `$TACK_MANIFEST`, which tests use to replace it), then `$TACK_HOME/tack.toml`
/// on top, validated.
pub fn load(paths: &Paths) -> Result<Manifest> {
    let mut m = match std::env::var_os("TACK_MANIFEST").filter(|v| !v.is_empty()) {
        Some(file) => parse_file(Path::new(&file))?,
        None => embedded()?,
    };
    let user = paths.user_manifest();
    if user.exists() {
        m.merge(parse_file(&user)?);
    }
    m.validate()?;
    Ok(m)
}

pub fn embedded() -> Result<Manifest> {
    let mut m: Manifest = toml::from_str(SKILLS).context("manifest/skills.toml")?;
    m.merge(toml::from_str(TOOLS).context("manifest/tools.toml")?);
    Ok(m)
}

fn parse_file(file: &Path) -> Result<Manifest> {
    let text =
        std::fs::read_to_string(file).with_context(|| format!("reading {}", file.display()))?;
    toml::from_str(&text).with_context(|| format!("parsing {}", file.display()))
}

impl Manifest {
    /// Adds `other`'s entries; one with an existing name replaces it (skills and tools share names).
    fn merge(&mut self, other: Manifest) {
        for s in other.skill {
            self.drop_name(&s.name);
            self.skill.push(s);
        }
        for t in other.tool {
            self.drop_name(&t.name);
            self.tool.push(t);
        }
        if other.local_model.is_some() {
            self.local_model = other.local_model;
        }
    }

    fn drop_name(&mut self, name: &str) {
        self.skill.retain(|s| s.name != name);
        self.tool.retain(|t| t.name != name);
    }

    pub fn validate(&self) -> Result<()> {
        let mut errs = Vec::new();
        let mut names = BTreeSet::from(["guard".to_string(), "rules".to_string()]);
        for s in &self.skill {
            let e = |msg: String| format!("skill {}: {msg}", s.name);
            check_common(
                &s.name,
                &s.description,
                &s.harnesses,
                &s.default,
                &mut names,
            )
            .into_iter()
            .for_each(|m| errs.push(e(m)));
            if !relative_inside(&s.path) {
                errs.push(e(format!("path {:?} must be relative, without ..", s.path)));
            }
            if s.is_own() {
                if store::OWN
                    .get_file(format!("{}/SKILL.md", s.path))
                    .is_none()
                {
                    errs.push(e(format!("skills/{}/SKILL.md is not embedded", s.path)));
                }
            } else {
                if !is_git_url(&s.source) {
                    errs.push(e(format!(
                        "source {:?} must be \"own\" or an https git URL",
                        s.source
                    )));
                }
                if s.rev.len() != 40 || !s.rev.chars().all(|c| c.is_ascii_hexdigit()) {
                    errs.push(e(format!("rev {:?} must be a 40-char commit SHA", s.rev)));
                }
            }
        }
        for t in &self.tool {
            let e = |msg: String| format!("tool {}: {msg}", t.name);
            check_common(
                &t.name,
                &t.description,
                &t.harnesses,
                &t.default,
                &mut names,
            )
            .into_iter()
            .for_each(|m| errs.push(e(m)));
            if t.base && !t.harnesses.is_empty() {
                errs.push(e("a base tool has harnesses = []".into()));
            }
            if !t.base && t.harnesses.is_empty() {
                errs.push(e("harnesses = [] needs base = true".into()));
            }
            if t.check.is_empty() {
                errs.push(e("check is empty".into()));
            }
            if !t.min_version.is_empty() && run::version_in(&t.min_version).is_none() {
                errs.push(e(format!("min_version {:?} is not N.N.N", t.min_version)));
            }
            for step in &t.install {
                if !["any", "macos", "linux"].contains(&step.os.as_str()) {
                    errs.push(e(format!(
                        "install os {:?} is not any/macos/linux",
                        step.os
                    )));
                }
                if step.run.is_empty() || step.run.iter().any(|argv| argv.is_empty()) {
                    errs.push(e("install run has an empty command".into()));
                }
            }
            for (key, cmds) in t.enable.iter().chain(&t.disable) {
                if !t.harnesses.contains(key) {
                    errs.push(e(format!("enable/disable key {key:?} is not in harnesses")));
                }
                if cmds.iter().any(|argv| argv.is_empty()) {
                    errs.push(e("enable/disable has an empty command".into()));
                }
            }
            for (key, file) in &t.marker {
                if !t.harnesses.contains(key) {
                    errs.push(e(format!("marker key {key:?} is not in harnesses")));
                }
                if !relative_inside(file) {
                    errs.push(e(format!("marker {file:?} must be relative, without ..")));
                }
            }
        }
        for s in &self.skill {
            if !s.group.is_empty() && names.contains(&s.group) {
                errs.push(format!(
                    "skill {}: group {:?} clashes with a component name",
                    s.name, s.group
                ));
            }
        }
        if let Some(lm) = &self.local_model {
            let mut e = |msg: &str| errs.push(format!("local_model {}: {msg}", lm.name));
            if lm.up.is_empty() || lm.up.iter().any(String::is_empty) {
                e("up must be a non-empty argv");
            }
            if lm.down.iter().any(String::is_empty) {
                e("down has an empty argument");
            }
            if !lm.base_url.starts_with("http://") && !lm.base_url.starts_with("https://") {
                e("base_url must start with http:// or https://");
            }
            if lm.model.is_empty() || lm.context == 0 {
                e("model must be set and context above 0");
            }
        }
        if !errs.is_empty() {
            bail!("invalid manifest:\n  {}", errs.join("\n  "));
        }
        Ok(())
    }
}

fn check_common(
    name: &str,
    description: &str,
    harnesses: &[String],
    default: &[String],
    names: &mut BTreeSet<String>,
) -> Vec<String> {
    let mut errs = Vec::new();
    let name_ok = !name.is_empty()
        && !name.starts_with('.')
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c));
    if !name_ok {
        errs.push("name must be letters, digits, - _ . and not start with .".into());
    }
    if !names.insert(name.to_string()) {
        errs.push("name is used twice (or is reserved)".into());
    }
    if description.chars().count() > 60 {
        errs.push("description is over 60 chars".into());
    }
    for h in harnesses {
        if Harness::parse(h).is_none() {
            errs.push(format!("unknown harness {h:?}"));
        }
    }
    for h in default {
        if !harnesses.contains(h) {
            errs.push(format!("default {h:?} is not in harnesses"));
        }
    }
    errs
}

fn is_git_url(source: &str) -> bool {
    // Absolute paths are accepted so tests can use a local repo as the source.
    source.starts_with("https://") || source.starts_with("file://") || source.starts_with('/')
}

fn relative_inside(path: &str) -> bool {
    let p = Path::new(path);
    !path.is_empty() && p.components().all(|c| matches!(c, PathPart::Normal(_)))
}

pub fn harnesses(names: &[String]) -> Vec<Harness> {
    names.iter().filter_map(|h| Harness::parse(h)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_catalogue_is_valid() {
        let m = embedded().unwrap();
        m.validate().unwrap();
        assert!(!m.skill.is_empty() && !m.tool.is_empty());
        let no_manual: Vec<&str> = m
            .tool
            .iter()
            .filter(|t| t.manual.is_empty())
            .map(|t| t.name.as_str())
            .collect();
        assert!(no_manual.is_empty(), "tools without manual: {no_manual:?}");
    }

    #[test]
    fn pi_stays_lean_by_default() {
        let m = embedded().unwrap();
        let sp: Vec<&Skill> = m
            .skill
            .iter()
            .filter(|s| s.group == "superpowers")
            .collect();
        assert!(!sp.is_empty());
        for s in sp {
            assert_eq!(s.default, ["claude"], "{}", s.name);
        }
        let tgrep = m.tool.iter().find(|t| t.name == "tgrep").unwrap();
        assert!(tgrep.default.is_empty());
    }

    #[test]
    fn bank_is_off_everywhere() {
        let m = embedded().unwrap();
        let bank: Vec<&Skill> = m.skill.iter().filter(|s| s.group == "bank").collect();
        assert!(!bank.is_empty());
        for s in bank {
            assert!(s.default.is_empty(), "{}", s.name);
        }
        let mut names: Vec<&str> = m.skill.iter().map(|s| s.name.as_str()).collect();
        names.extend(m.tool.iter().map(|t| t.name.as_str()));
        let n = names.len();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), n, "duplicate component names");
    }

    fn parse(text: &str) -> Manifest {
        toml::from_str(text).unwrap()
    }

    const OK: &str = r#"
        [[skill]]
        name = "a"
        description = "d"
        source = "https://example.com/r.git"
        path = "a"
        rev = "0123456789abcdef0123456789abcdef01234567"
        harnesses = ["claude", "pi"]
        default = ["pi"]
        [[tool]]
        name = "t"
        description = "d"
        harnesses = ["pi"]
        check = ["t"]
        [tool.enable]
        pi = [["t", "on"]]
        [tool.marker]
        pi = "extensions/t.ts"
    "#;

    #[test]
    fn validation_catches_bad_entries() {
        parse(OK).validate().unwrap();
        let bad = [
            (
                "rev = \"0123456789abcdef0123456789abcdef01234567\"",
                "rev = \"abc\"",
            ),
            ("default = [\"pi\"]", "default = [\"codex\"]"),
            (
                "harnesses = [\"claude\", \"pi\"]",
                "harnesses = [\"claude\", \"x\"]",
            ),
            ("name = \"t\"", "name = \"a\""),
            ("name = \"t\"", "name = \"guard\""),
            ("path = \"a\"", "path = \"../a\""),
            (
                "source = \"https://example.com/r.git\"",
                "source = \"git@x:r\"",
            ),
            ("pi = [[\"t\", \"on\"]]", "claude = [[\"t\"]]"),
            ("pi = \"extensions/t.ts\"", "pi = \"/abs/t.ts\""),
            ("pi = \"extensions/t.ts\"", "pi = \"../t.ts\""),
            ("pi = \"extensions/t.ts\"", "claude = \"t.ts\""),
        ];
        for (from, to) in bad {
            let text = OK.replacen(from, to, 1);
            assert!(parse(&text).validate().is_err(), "accepted: {to}");
        }
    }

    #[test]
    fn user_entries_replace_by_name() {
        let mut m = parse(OK);
        m.merge(parse(
            "[[tool]]\nname = \"a\"\ndescription = \"x\"\nbase = true\ncheck = [\"a\"]",
        ));
        assert!(m.skill.is_empty());
        assert_eq!(m.tool.len(), 2);
        m.validate().unwrap();
    }

    const LOCAL: &str = "[local_model]\nname = \"Q\"\nup = [\"/bin/true\"]\nbase_url = \"http://127.0.0.1:8080/v1\"\nmodel = \"q\"\ncontext = 131072\n";

    #[test]
    fn local_model_comes_from_the_user_file_and_is_checked() {
        assert!(embedded().unwrap().local_model.is_none());
        let mut m = parse(OK);
        m.merge(parse(LOCAL));
        assert_eq!(m.local_model.as_ref().unwrap().context, 131072);
        m.validate().unwrap();
        m.merge(parse(
            &LOCAL
                .replace("http://", "ftp://")
                .replace("[\"/bin/true\"]", "[]"),
        ));
        let err = m.validate().unwrap_err().to_string();
        assert!(err.contains("up must be a non-empty argv"), "{err}");
        assert!(err.contains("base_url must start with http"), "{err}");
    }
}
