use std::path::PathBuf;

use anyhow::{bail, Result};

use crate::link::{self, Change};
use crate::manifest::{self, InstallStep, Manifest, Skill, Tool};
use crate::paths::{Harness, Paths};
use crate::run::{self, Version};
use crate::state::State;
use crate::{claude, pi, rules, store};

pub enum Kind<'a> {
    Guard,
    Rules,
    Skill(&'a Skill),
    Tool(&'a Tool),
}

/// Anything with an on/off state: the builtin guard and rules, every skill, every non-base tool.
pub struct Component<'a> {
    pub name: &'a str,
    pub description: &'a str,
    pub harnesses: Vec<Harness>,
    pub default: Vec<Harness>,
    pub group: &'a str,
    pub kind: Kind<'a>,
}

pub fn all(m: &Manifest) -> Vec<Component<'_>> {
    let mut out = vec![Component {
        name: "guard",
        description: "Blocks secret reads, force-push and rm -rf ~",
        harnesses: Harness::ALL.to_vec(),
        default: Harness::ALL.to_vec(),
        group: "",
        kind: Kind::Guard,
    }];
    out.push(Component {
        name: "rules",
        description: "Always-on reply rules: short, plain, goal-first",
        harnesses: Harness::ALL.to_vec(),
        default: Harness::ALL.to_vec(),
        group: "",
        kind: Kind::Rules,
    });
    out.extend(m.skill.iter().map(|s| Component {
        name: &s.name,
        description: &s.description,
        harnesses: manifest::harnesses(&s.harnesses),
        default: manifest::harnesses(&s.default),
        group: &s.group,
        kind: Kind::Skill(s),
    }));
    out.extend(m.tool.iter().filter(|t| !t.base).map(|t| Component {
        name: &t.name,
        description: &t.description,
        harnesses: manifest::harnesses(&t.harnesses),
        default: manifest::harnesses(&t.default),
        group: "",
        kind: Kind::Tool(t),
    }));
    out
}

pub fn link_path(paths: &Paths, name: &str, h: Harness) -> PathBuf {
    paths.skills_dir(h).join(name)
}

pub enum Outcome {
    Changed,
    Same,
    /// Something tack did not create sits where the skill link goes.
    Foreign(PathBuf),
}

/// Makes the on-disk state of one component for one harness match `on`. For a tool, on runs its
/// enable steps and off runs its disable steps only if tack wired it; `state` records which.
pub fn set(
    paths: &Paths,
    c: &Component,
    h: Harness,
    on: bool,
    state: &mut State,
) -> Result<Outcome> {
    match c.kind {
        Kind::Guard => {
            let changed = match (h, on) {
                (Harness::Claude, true) => claude::install_hook(paths)?,
                (Harness::Claude, false) => claude::remove_hook(paths)?,
                (Harness::Pi, true) => pi::install_extension(paths)?,
                (Harness::Pi, false) => pi::remove_extension(paths)?,
            };
            Ok(if changed {
                Outcome::Changed
            } else {
                Outcome::Same
            })
        }
        Kind::Rules => Ok(if rules::set(paths, h, on)? {
            Outcome::Changed
        } else {
            Outcome::Same
        }),
        Kind::Skill(s) => {
            let link = link_path(paths, c.name, h);
            let change = if on {
                link::ensure(&link, &store::skill_dir(paths, s), &paths.tack)?
            } else {
                link::remove(&link, &paths.tack)?
            };
            Ok(match change {
                Change::Done => Outcome::Changed,
                Change::Same => Outcome::Same,
                Change::Foreign => Outcome::Foreign(link),
            })
        }
        Kind::Tool(t) => {
            if on {
                ensure_tool(t)?;
            } else if !state.wired(c.name, h) || matches!(tool_status(t), ToolStatus::Missing) {
                // Wiring tack never did (or a program that is gone) is not tack's to undo.
                state.set_wired(c.name, h, false);
                return Ok(Outcome::Same);
            }
            let steps = if on { &t.enable } else { &t.disable };
            let cmds = steps.get(h.name()).map(Vec::as_slice).unwrap_or_default();
            run::run_all(cmds)?;
            state.set_wired(c.name, h, on && !cmds.is_empty());
            // tack cannot tell whether the steps changed anything, so it never reports a change.
            Ok(Outcome::Same)
        }
    }
}

/// The marker file `t` declares for `h` that is missing, if any.
pub fn missing_marker(paths: &Paths, t: &Tool, h: Harness) -> Option<PathBuf> {
    let file = paths.config_dir(h).join(t.marker.get(h.name())?);
    (!file.exists()).then_some(file)
}

/// Wired per state, and its marker file (when declared) is still there.
pub fn wired(paths: &Paths, t: &Tool, h: Harness, state: &State) -> bool {
    state.wired(&t.name, h) && missing_marker(paths, t, h).is_none()
}

pub enum ToolStatus {
    Ok(Option<Version>),
    Missing,
    Old(Option<Version>),
}

pub fn tool_status(t: &Tool) -> ToolStatus {
    let Ok(out) = run::run(&t.check) else {
        return ToolStatus::Missing;
    };
    let v = run::version_in(&out);
    match run::version_in(&t.min_version) {
        Some(min) if v.is_none_or(|v| v < min) => ToolStatus::Old(v),
        _ => ToolStatus::Ok(v),
    }
}

pub fn describe(t: &Tool, s: &ToolStatus) -> String {
    match s {
        ToolStatus::Ok(Some(v)) => format!("{} {}", t.name, run::show(*v)),
        ToolStatus::Ok(None) => format!("{} ok", t.name),
        ToolStatus::Missing => format!("{} missing", t.name),
        ToolStatus::Old(v) => format!(
            "{} {} (needs {}+)",
            t.name,
            v.map(run::show).unwrap_or_else(|| "?".into()),
            t.min_version
        ),
    }
}

/// The first install step whose `os` and `needs` hold on this machine.
pub fn install_step(t: &Tool) -> Option<&InstallStep> {
    let os = if cfg!(target_os = "macos") {
        "macos"
    } else {
        "linux"
    };
    t.install
        .iter()
        .find(|s| (s.os == "any" || s.os == os) && s.needs.iter().all(|n| run::which(n).is_some()))
}

/// One command (or the manual route) that gets this tool working.
pub fn tool_fix(t: &Tool) -> String {
    if install_step(t).is_some() {
        "tack install".into()
    } else if !t.manual.is_empty() {
        t.manual.clone()
    } else {
        format!("install {} by hand", t.name)
    }
}

/// Installs `t` when its check fails. Prints a line (and the hint) when it installs something.
pub fn ensure_tool(t: &Tool) -> Result<()> {
    if matches!(tool_status(t), ToolStatus::Ok(_)) {
        return Ok(());
    }
    let Some(step) = install_step(t) else {
        bail!("{} is missing; fix: {}", t.name, tool_fix(t));
    };
    run::run_all(&step.run)?;
    match tool_status(t) {
        ToolStatus::Ok(_) => {
            println!("installed {}", t.name);
            if !t.hint.is_empty() {
                println!("  {}", t.hint);
            }
            Ok(())
        }
        s => bail!("{} after install", describe(t, &s)),
    }
}
