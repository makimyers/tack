use std::path::Path;
use std::process::Command;

use serde::Serialize;

use crate::components::{self, Kind, ToolStatus};
use crate::link::{self, Status as LinkStatus};
use crate::manifest::{self, Manifest};
use crate::paths::{Harness, Paths};
use crate::state::State;
use crate::{claude, pi, rules, run, store};

const NODE_MIN: run::Version = [22, 19, 0];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Ok,
    Warn,
    Fail,
}

#[derive(Debug, Serialize)]
pub struct Check {
    pub name: String,
    pub status: Status,
    pub detail: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub fix: String,
}

pub fn check(status: Status, name: &str, detail: String, fix: String) -> Check {
    Check {
        name: name.to_string(),
        status,
        detail,
        fix,
    }
}

fn ok(name: &str, detail: String) -> Check {
    check(Status::Ok, name, detail, String::new())
}

/// `~/x` for paths under home, so lines stay short.
pub fn tilde(home: &Path, p: &Path) -> String {
    match p.strip_prefix(home) {
        Ok(rest) => format!("~/{}", rest.display()),
        Err(_) => p.display().to_string(),
    }
}

/// git, Node.js >= 22.19 and npm. tack never installs these.
pub fn prereqs() -> Vec<Check> {
    let mac = cfg!(target_os = "macos");
    let git_fix = if mac {
        "xcode-select --install"
    } else {
        "sudo apt-get install -y git"
    };
    let node_fix = if mac {
        "brew install node"
    } else {
        "curl -fsSL https://deb.nodesource.com/setup_22.x | sudo -E bash - && sudo apt-get install -y nodejs"
    };
    let version = |cmd: &str| {
        run::run(&[cmd.to_string(), "--version".to_string()])
            .ok()
            .map(|out| run::version_in(&out))
    };
    let missing = |name: &str, fix: &str| {
        check(
            Status::Fail,
            name,
            format!("{name} is not on PATH"),
            fix.into(),
        )
    };
    let git = match version("git") {
        Some(_) => ok("git", "found".into()),
        None => missing("git", git_fix),
    };
    let node = match version("node") {
        Some(Some(v)) if v >= NODE_MIN => ok("node", run::show(v)),
        Some(v) => check(
            Status::Fail,
            "node",
            format!(
                "node {} is older than {}",
                v.map(run::show).unwrap_or_else(|| "?".into()),
                run::show(NODE_MIN)
            ),
            node_fix.into(),
        ),
        None => missing("node", node_fix),
    };
    let npm = match version("npm") {
        Some(_) => ok("npm", "found".into()),
        None => missing("npm", node_fix),
    };
    vec![git, node, npm]
}

pub fn run(paths: &Paths, m: &Manifest, state: &State) -> Vec<Check> {
    let mut out = prereqs();

    for t in &m.tool {
        let enabled = manifest::harnesses(&t.harnesses)
            .into_iter()
            .any(|h| state.get(&t.name, h));
        if !t.base && !enabled {
            continue;
        }
        let s = components::tool_status(t);
        let detail = components::describe(t, &s);
        out.push(match s {
            ToolStatus::Ok(_) => ok(&t.name, detail),
            _ if t.required => check(Status::Fail, &t.name, detail, components::tool_fix(t)),
            _ => check(Status::Warn, &t.name, detail, components::tool_fix(t)),
        });
    }

    let home = &paths.home;
    let bin = paths.tack_bin();
    out.push(if bin.is_file() {
        ok("tack", tilde(home, &bin))
    } else {
        check(
            Status::Fail,
            "tack",
            format!("{} is missing", tilde(home, &bin)),
            "tack install".into(),
        )
    });
    let local = paths.local_bin().join("tack");
    out.push(link_check(paths, "tack link", &local, |t| t == bin));
    if let Some(lm) = &m.local_model {
        out.push(local_model_check(paths, lm));
    }
    let dependency_script = paths.tack.join("bin/check-dependencies");
    if dependency_script.symlink_metadata().is_ok() {
        check_dependencies(&dependency_script, &mut out);
    }

    for c in components::all(m) {
        for &h in &c.harnesses {
            if !state.get(c.name, h) {
                continue;
            }
            let name = format!("{} ({})", c.name, h.name());
            match c.kind {
                Kind::Skill(s) => {
                    let want = store::skill_dir(paths, s);
                    let link = components::link_path(paths, c.name, h);
                    out.push(link_check(paths, &name, &link, |t| {
                        t == want && t.join("SKILL.md").is_file()
                    }));
                }
                Kind::Guard => {
                    let (installed, what) = match h {
                        Harness::Claude => (claude::hook_installed(paths), "Claude Code hook"),
                        Harness::Pi => (pi::extension_installed(paths), "pi extension"),
                    };
                    out.push(if installed {
                        ok(&name, format!("{what} installed"))
                    } else {
                        check(
                            Status::Fail,
                            &name,
                            format!("{what} is not installed"),
                            format!("tack on guard --{}", h.name()),
                        )
                    });
                }
                Kind::Rules if !rules::installed(paths, h) => out.push(check(
                    Status::Warn,
                    &name,
                    format!(
                        "{} lacks the current tack rules block",
                        tilde(home, &paths.rules_file(h))
                    ),
                    "tack install".into(),
                )),
                Kind::Rules if h == Harness::Pi && !rules::extension_installed(paths) => {
                    out.push(check(
                        Status::Warn,
                        &name,
                        format!(
                            "{} is missing or out of date",
                            tilde(home, &paths.pi_rules_extension())
                        ),
                        "tack install".into(),
                    ))
                }
                Kind::Rules => out.push(ok(&name, "rules block current".into())),
                Kind::Tool(t) if state.wired(c.name, h) => {
                    if let Some(file) = components::missing_marker(paths, t, h) {
                        out.push(check(
                            Status::Warn,
                            &name,
                            format!("{} is missing", tilde(home, &file)),
                            "tack install".into(),
                        ));
                    }
                }
                Kind::Tool(_) => {}
            }
        }
    }
    out
}

fn link_check(paths: &Paths, name: &str, link: &Path, good: impl Fn(&Path) -> bool) -> Check {
    let shown = tilde(&paths.home, link);
    match link::status(link, &paths.tack) {
        LinkStatus::Ours(t) if good(&t) => ok(name, shown),
        LinkStatus::Foreign => check(
            Status::Warn,
            name,
            format!("{shown} exists (not tack's)"),
            format!("mv {shown} {shown}.bak && tack install"),
        ),
        _ => check(
            Status::Fail,
            name,
            format!("{shown} is missing or broken"),
            "tack install".into(),
        ),
    }
}

pub fn failed(checks: &[Check]) -> bool {
    checks.iter().any(|c| c.status == Status::Fail)
}

fn check_dependencies(script: &Path, checks: &mut Vec<Check>) {
    let start = checks.len();
    let fix = "see machines/bazzite/README.md in the Tack checkout";
    match Command::new(script).arg("check").output() {
        Ok(output) => {
            for line in String::from_utf8_lossy(&output.stdout).lines() {
                let mut fields = line.splitn(3, '\t');
                match (fields.next(), fields.next(), fields.next()) {
                    (Some("ok"), Some(name), Some(detail)) => checks.push(ok(name, detail.into())),
                    (Some("fail"), Some(name), Some(detail)) => {
                        checks.push(check(Status::Fail, name, detail.into(), fix.into()));
                    }
                    _ => checks.push(check(
                        Status::Fail,
                        "dependency lock",
                        format!("unexpected checker output: {line}"),
                        fix.into(),
                    )),
                }
            }
            if checks.len() == start || !output.status.success() && !failed(&checks[start..]) {
                checks.push(check(
                    Status::Fail,
                    "dependency lock",
                    format!(
                        "checker returned {}: {}",
                        output.status,
                        String::from_utf8_lossy(&output.stderr).trim()
                    ),
                    fix.into(),
                ));
            }
        }
        Err(error) => checks.push(check(
            Status::Fail,
            "dependency lock",
            format!("could not run checker: {error}"),
            fix.into(),
        )),
    }
}

/// Prints every check that is not ok, with its fix, then a count line.
pub fn print(checks: &[Check]) {
    for c in checks.iter().filter(|c| c.status != Status::Ok) {
        let label = if c.status == Status::Fail {
            "fail"
        } else {
            "warn"
        };
        println!("{label}: {}: {}", c.name, c.detail);
        if !c.fix.is_empty() {
            println!("  fix: {}", c.fix);
        }
    }
    let count = |s| checks.iter().filter(|c| c.status == s).count();
    println!(
        "doctor: {} ok, {} warn, {} fail",
        count(Status::Ok),
        count(Status::Warn),
        count(Status::Fail)
    );
}

fn local_model_check(paths: &Paths, lm: &manifest::LocalModel) -> Check {
    let name = "local model";
    let user = tilde(&paths.home, &paths.user_manifest());
    let prog = Path::new(&lm.up[0]);
    if prog.is_absolute() && !prog.is_file()
        || !prog.is_absolute() && run::which(&lm.up[0]).is_none()
    {
        return check(
            Status::Fail,
            name,
            format!("{} is missing", lm.up[0]),
            format!("fix `up` in {user}"),
        );
    }
    let settings = claude::read(&paths.config_dir(Harness::Pi).join("settings.json"));
    let default = settings.ok().flatten().is_some_and(|s| {
        s["defaultProvider"] == crate::local::PROVIDER && s["defaultModel"] == lm.model.as_str()
    });
    if default {
        ok(name, format!("{} is pi's default", lm.name))
    } else {
        check(
            Status::Warn,
            name,
            format!("pi's default model is not {}", lm.name),
            "tack install".into(),
        )
    }
}
