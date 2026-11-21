mod claude;
mod components;
mod doctor;
mod guard;
mod link;
mod local;
mod manifest;
mod paths;
mod pi;
mod rules;
mod run;
mod state;
mod store;
mod t3;

use std::collections::BTreeSet;
use std::fs;
use std::io::Read;
use std::path::Path;
use std::process::Command;

use anyhow::{anyhow, bail, Context, Result};
use clap::{Parser, Subcommand};

use components::{Component, Kind, Outcome, ToolStatus};
use doctor::tilde;
use link::Change;
use paths::{Harness, Paths};
use state::State;

#[derive(Parser)]
#[command(
    name = "tack",
    version,
    about = "Installs and manages a T3 Code + pi + Claude Code setup"
)]
struct Cli {
    #[command(subcommand)]
    cmd: Option<Cmd>,
}

#[derive(Subcommand)]
enum Cmd {
    /// Install or repair everything; safe to re-run
    Install {
        /// Skip every tool install and enable step
        #[arg(long)]
        skip_tools: bool,
    },
    /// Show every component and where it is on
    List {
        /// Show each group member on its own row
        #[arg(long)]
        all: bool,
    },
    /// Turn components or groups on
    On(Toggle),
    /// Turn components or groups off
    Off(Toggle),
    /// Check the setup; each failure prints one fix
    Doctor {
        #[arg(long)]
        json: bool,
    },
    /// Start T3 Code
    Open,
    /// Switch the local model on or off, or show whether it runs
    Model {
        #[arg(value_parser = ["on", "off", "status"], default_value = "status")]
        action: String,
    },
    /// Remove everything tack created (pi, T3 Code and Claude Code stay)
    Uninstall,
    #[command(hide = true)]
    Guard {
        #[arg(long, value_parser = ["claude", "pi"])]
        harness: String,
    },
}

#[derive(clap::Args)]
struct Toggle {
    /// Component or group names
    #[arg(required = true)]
    names: Vec<String>,
    /// Only Claude Code
    #[arg(long)]
    claude: bool,
    /// Only pi
    #[arg(long)]
    pi: bool,
}

fn main() {
    let cli = Cli::parse();
    let code = match cli.cmd {
        Some(Cmd::Guard { harness }) => guard_cmd(&harness),
        cmd => match Paths::from_env().and_then(|p| dispatch(&p, cmd)) {
            Ok(code) => code,
            Err(e) => {
                eprintln!("tack: {e:#}");
                1
            }
        },
    };
    std::process::exit(code);
}

fn dispatch(paths: &Paths, cmd: Option<Cmd>) -> Result<i32> {
    // Links point into $TACK_HOME and uninstall deletes it, so it must be a plain absolute path.
    if !paths.tack.is_absolute() || paths.tack.components().any(|c| c.as_os_str() == "..") {
        eprintln!("tack: TACK_HOME must be an absolute path without ..");
        return Ok(2);
    }
    match cmd {
        None => list(paths, false, true),
        Some(Cmd::Install { skip_tools }) => install(paths, skip_tools),
        Some(Cmd::List { all }) => list(paths, all, false),
        Some(Cmd::On(t)) => toggle(paths, t, true),
        Some(Cmd::Off(t)) => toggle(paths, t, false),
        Some(Cmd::Doctor { json }) => doctor_cmd(paths, json),
        Some(Cmd::Open) => open(paths),
        Some(Cmd::Model { action }) => model_cmd(paths, &action),
        Some(Cmd::Uninstall) => uninstall(paths),
        Some(Cmd::Guard { harness }) => Ok(guard_cmd(&harness)),
    }
}

/// Hook entry point. Kept apart from manifest loading so a broken tack.toml cannot open the gate.
fn guard_cmd(harness: &str) -> i32 {
    let Some(home) = std::env::var_os("HOME") else {
        eprintln!("Blocked by tack guard: HOME is not set");
        return 2;
    };
    let mut input = String::new();
    if std::io::stdin().read_to_string(&mut input).is_err() {
        input.clear();
    }
    let h = Harness::parse(harness).unwrap_or(Harness::Claude);
    let (code, msg) = guard::run(h, &input, Path::new(&home));
    if !msg.is_empty() {
        eprintln!("{msg}");
    }
    code
}

fn load_state(paths: &Paths, comps: &[Component]) -> Result<State> {
    let file = paths.state_file();
    let s = State::load(&file).map_err(|e| anyhow!("{e:#}\n  fix: {}", state::fix(&file)))?;
    Ok(with_defaults(s, comps))
}

fn with_defaults(mut s: State, comps: &[Component]) -> State {
    for c in comps {
        s.add_default(c.name, &c.harnesses, &c.default);
    }
    s
}

fn on_off(on: bool) -> &'static str {
    if on {
        "on"
    } else {
        "off"
    }
}

fn install(paths: &Paths, skip_tools: bool) -> Result<i32> {
    let pre = doctor::prereqs();
    if doctor::failed(&pre) {
        for c in pre.iter().filter(|c| c.status == doctor::Status::Fail) {
            eprintln!("missing prerequisite: {}", c.detail);
            eprintln!("  fix: {}", c.fix);
        }
        return Ok(2);
    }
    let m = manifest::load(paths)?;
    let comps = components::all(&m);
    let mut failed = false;

    self_install(paths)?;
    let mut state = load_state(paths, &comps)?;
    state.save(&paths.state_file())?;

    if !skip_tools {
        for t in m.tool.iter().filter(|t| t.base) {
            if let Err(e) = components::ensure_tool(t) {
                println!("{}: {e:#}", if t.required { "fail" } else { "warn" });
                failed |= t.required;
            }
        }
    }
    match t3::set_providers(paths) {
        Ok(changed) if !changed.is_empty() => {
            println!("t3: {} (restart T3 Code to apply)", changed.join(", "))
        }
        Ok(_) => {}
        Err(e) => println!("warn: {e:#}"),
    }
    if let Some(lm) = &m.local_model {
        match local::wire_pi(paths, lm) {
            Ok(true) => println!("pi: default model {} ({})", lm.name, lm.base_url),
            Ok(false) => {}
            Err(e) => println!("warn: {e:#}"),
        }
        match local::wire_t3(paths) {
            Ok(true) => println!("t3: Local model on / off / status in the scripts menu"),
            Ok(false) => {}
            Err(e) => println!("warn: {e:#}"),
        }
    }

    store::write_own(paths)?;
    let wanted = m.skill.iter().filter(|s| {
        manifest::harnesses(&s.harnesses)
            .into_iter()
            .any(|h| state.get(&s.name, h))
    });
    let fetch_failed = store::fetch_all(paths, wanted);
    failed |= !fetch_failed.is_empty();

    for c in &comps {
        for &h in &c.harnesses {
            let on = state.get(c.name, h);
            let result = match c.kind {
                Kind::Tool(_) if skip_tools || !on => continue,
                // Re-running enable steps could undo the owner's own edits to that wiring, so a
                // wired tool only gets its program checked.
                Kind::Tool(t) if components::wired(paths, t, h, &state) => {
                    components::ensure_tool(t).map(|_| Outcome::Same)
                }
                _ if on && fetch_failed.contains(c.name) => continue,
                _ => components::set(paths, c, h, on, &mut state),
            };
            match result {
                Ok(Outcome::Changed) => println!("{}: {} for {}", c.name, on_off(on), h.name()),
                Ok(Outcome::Same) => {}
                Ok(Outcome::Foreign(p)) if on => println!(
                    "warn: {} exists (not tack's); left alone",
                    tilde(&paths.home, &p)
                ),
                Ok(Outcome::Foreign(_)) => {}
                Err(e) => {
                    let optional = matches!(c.kind, Kind::Tool(t) if !t.required);
                    let label = if optional { "warn" } else { "fail" };
                    println!("{label}: {} ({}): {e:#}", c.name, h.name());
                    failed |= !optional;
                }
            }
        }
    }
    state.save(&paths.state_file())?;

    for h in Harness::ALL {
        let keep: BTreeSet<&str> = comps
            .iter()
            .filter(|c| matches!(c.kind, Kind::Skill(_)) && c.harnesses.contains(&h))
            .filter(|c| state.get(c.name, h))
            .map(|c| c.name)
            .collect();
        for p in link::prune(&paths.skills_dir(h), &paths.tack, |n| keep.contains(n))? {
            println!("removed stale link {}", tilde(&paths.home, &p));
        }
    }

    if failed {
        println!("install finished with failures; run `tack doctor`");
        Ok(1)
    } else {
        println!("install done; next: tack open");
        Ok(0)
    }
}

/// Copies the running binary to `$TACK_HOME/bin/tack` and links `~/.local/bin/tack` to it.
fn self_install(paths: &Paths) -> Result<()> {
    let exe = std::env::current_exe()?.canonicalize()?;
    let bin = paths.tack_bin();
    if bin.canonicalize().ok().as_ref() != Some(&exe) {
        fs::create_dir_all(bin.with_file_name(""))?;
        // Copy then rename, so a running copy of the old binary is never overwritten in place.
        let tmp = bin.with_extension("new");
        fs::copy(&exe, &tmp).with_context(|| format!("copying tack to {}", tmp.display()))?;
        fs::rename(&tmp, &bin)?;
    }
    let local = paths.local_bin().join("tack");
    match link::ensure(&local, &bin, &paths.tack)? {
        Change::Done => println!("linked {}", tilde(&paths.home, &local)),
        Change::Same => {}
        Change::Foreign => println!(
            "warn: {} exists (not tack's); left alone",
            tilde(&paths.home, &local)
        ),
    }
    let on_path = std::env::var_os("PATH")
        .is_some_and(|p| std::env::split_paths(&p).any(|d| d == paths.local_bin()));
    if !on_path {
        println!("add this line to your shell profile: export PATH=\"$HOME/.local/bin:$PATH\"");
    }
    Ok(())
}

fn toggle(paths: &Paths, t: Toggle, on: bool) -> Result<i32> {
    let m = manifest::load(paths)?;
    let comps = components::all(&m);
    let mut state = load_state(paths, &comps)?;
    let flags: Vec<Harness> = [(t.claude, Harness::Claude), (t.pi, Harness::Pi)]
        .into_iter()
        .filter_map(|(set, h)| set.then_some(h))
        .collect();

    let mut targets: Vec<(&Component, Vec<Harness>)> = Vec::new();
    for name in &t.names {
        if let Some(c) = comps.iter().find(|c| c.name == name) {
            if let Some(h) = flags.iter().find(|h| !c.harnesses.contains(h)) {
                eprintln!("tack: {name} does not support {}", h.name());
                return Ok(2);
            }
            let hs = if flags.is_empty() {
                c.harnesses.clone()
            } else {
                flags.clone()
            };
            targets.push((c, hs));
            continue;
        }
        let members: Vec<&Component> = comps
            .iter()
            .filter(|c| !c.group.is_empty() && c.group == name)
            .collect();
        if members.is_empty() {
            if m.tool.iter().any(|t| t.base && &t.name == name) {
                eprintln!("tack: {name} is a base tool; it is always on");
            } else {
                eprintln!("tack: no component or group named {name}; see `tack list --all`");
            }
            return Ok(2);
        }
        for c in members {
            let hs = c
                .harnesses
                .iter()
                .copied()
                .filter(|h| flags.is_empty() || flags.contains(h))
                .collect();
            targets.push((c, hs));
        }
    }

    for (c, hs) in &targets {
        for &h in hs {
            state.set(c.name, h, on);
        }
    }
    state.save(&paths.state_file())?;

    let mut failed = false;
    for (c, hs) in &targets {
        if let (true, Kind::Skill(s)) = (on, &c.kind) {
            if let Err(e) = store::ensure_skill(paths, s) {
                println!("fail: {}: {e:#}", c.name);
                failed = true;
                continue;
            }
        }
        for &h in hs {
            match components::set(paths, c, h, on, &mut state) {
                Ok(Outcome::Foreign(p)) => println!(
                    "warn: {} exists (not tack's); left alone",
                    tilde(&paths.home, &p)
                ),
                Ok(_) => println!("{}: {} for {}", c.name, on_off(on), h.name()),
                Err(e) => {
                    println!("fail: {} ({}): {e:#}", c.name, h.name());
                    failed = true;
                }
            }
        }
    }
    state.save(&paths.state_file())?;
    Ok(failed as i32)
}

fn list(paths: &Paths, all: bool, next: bool) -> Result<i32> {
    let m = manifest::load(paths)?;
    let comps = components::all(&m);
    let state = load_state(paths, &comps)?;
    if !paths.state_file().exists() {
        println!("not installed yet; this is what tack install will turn on");
    }
    let cell = |c: &Component, h: Harness| -> &'static str {
        if !c.harnesses.contains(&h) {
            "-"
        } else if matches!(c.kind, Kind::Skill(_))
            && link::status(&components::link_path(paths, c.name, h), &paths.tack)
                == link::Status::Foreign
        {
            "exists"
        } else {
            on_off(state.get(c.name, h))
        }
    };

    let mut rows: Vec<[String; 4]> = Vec::new();
    let mut groups = BTreeSet::new();
    for c in &comps {
        if all || c.group.is_empty() {
            rows.push([
                c.name.into(),
                cell(c, Harness::Claude).into(),
                cell(c, Harness::Pi).into(),
                c.description.into(),
            ]);
        } else if groups.insert(c.group) {
            let members: Vec<&Component> = comps.iter().filter(|o| o.group == c.group).collect();
            let group_cell = |h| {
                let cells: Vec<&str> = members
                    .iter()
                    .map(|o| cell(o, h))
                    .filter(|s| *s != "-")
                    .collect();
                let on = cells.iter().filter(|s| **s == "on").count();
                match on {
                    _ if cells.is_empty() => "-".to_string(),
                    0 => "off".to_string(),
                    n if n == cells.len() => "on".to_string(),
                    n => format!("{n}/{}", cells.len()),
                }
            };
            rows.push([
                c.group.into(),
                group_cell(Harness::Claude),
                group_cell(Harness::Pi),
                format!("group of {} skills", members.len()),
            ]);
        }
    }

    // Lines stay under 100 columns: name, two 6-wide cells, description clipped to what is left.
    let name_w = rows
        .iter()
        .map(|r| r[0].chars().count())
        .max()
        .unwrap_or(0)
        .clamp(4, 32);
    let desc_w = 99 - name_w - 15;
    let line = |r: [&str; 4]| {
        let text = format!(
            "{:<name_w$} {:<6} {:<6} {}",
            clip(r[0], name_w),
            r[1],
            r[2],
            clip(r[3], desc_w)
        );
        println!("{}", text.trim_end());
    };
    line(["name", "claude", "pi", "description"]);
    for r in &rows {
        line([&r[0], &r[1], &r[2], &r[3]]);
    }
    if rows.iter().any(|r| r[1] == "exists" || r[2] == "exists") {
        println!("exists = a file tack did not create has that name; tack leaves it alone");
    }

    let base: Vec<(bool, String)> = m
        .tool
        .iter()
        .filter(|t| t.base)
        .map(|t| {
            let s = components::tool_status(t);
            let broken = t.required && !matches!(s, ToolStatus::Ok(_));
            (broken, components::describe(t, &s))
        })
        .collect();
    println!(
        "base: {}",
        base.iter()
            .map(|(_, d)| d.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    );

    if next {
        let step = if !paths.tack_bin().is_file() {
            "tack install"
        } else if base.iter().any(|(broken, _)| *broken) {
            "tack doctor"
        } else {
            "tack open"
        };
        println!("next: {step}");
    }
    Ok(0)
}

fn clip(s: &str, w: usize) -> String {
    if s.chars().count() <= w {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(w.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}

fn doctor_cmd(paths: &Paths, json: bool) -> Result<i32> {
    let m = manifest::load(paths)?;
    let comps = components::all(&m);
    let file = paths.state_file();
    let (state, broken) = match State::load(&file) {
        Ok(s) => (s, None),
        Err(e) => (State::default(), Some(format!("{e:#}"))),
    };
    let mut checks = doctor::run(paths, &m, &with_defaults(state, &comps));
    if let Some(detail) = broken {
        let fix = state::fix(&file);
        checks.push(doctor::check(doctor::Status::Fail, "state", detail, fix));
    }
    let failed = doctor::failed(&checks);
    if json {
        let out = serde_json::json!({ "ok": !failed, "checks": checks });
        println!("{}", serde_json::to_string_pretty(&out)?);
    } else {
        doctor::print(&checks);
    }
    Ok(failed as i32)
}

fn open(paths: &Paths) -> Result<i32> {
    if let Some(lm) = manifest::load(paths)?.local_model {
        if local::running(&lm) == Some(false) {
            println!("{} is off; switch it on from T3 Code's scripts menu (Local model: on) or `tack model on`", lm.name);
        }
    }
    match Command::new("t3").status() {
        Ok(s) => Ok(s.code().unwrap_or(1)),
        Err(_) => {
            eprintln!("tack: t3 (T3 Code) is not on PATH");
            eprintln!("  fix: tack install");
            Ok(1)
        }
    }
}

fn model_cmd(paths: &Paths, action: &str) -> Result<i32> {
    let Some(lm) = manifest::load(paths)?.local_model else {
        println!(
            "no local model set; add [local_model] to {}",
            tilde(&paths.home, &paths.user_manifest())
        );
        return Ok(1);
    };
    let ok = match action {
        "on" => local::run(&lm.up)?,
        "off" if lm.down.is_empty() => {
            println!(
                "[local_model] has no `down` command, so tack cannot stop {}",
                lm.name
            );
            return Ok(1);
        }
        "off" => local::run(&lm.down)?,
        _ => true,
    };
    match local::running(&lm) {
        Some(true) => println!("● {}: ON ({})", lm.name, lm.base_url),
        Some(false) => println!("○ {}: OFF", lm.name),
        None => println!("? {}: cannot check {}", lm.name, lm.base_url),
    }
    Ok(!ok as i32)
}

fn uninstall(paths: &Paths) -> Result<i32> {
    if paths.home.starts_with(&paths.tack) {
        bail!(
            "refusing to delete {}: it holds your home folder",
            paths.tack.display()
        );
    }
    // Everything below deletes, so $TACK_HOME must look like tack's own folder.
    if paths.tack.exists() && !paths.tack_bin().is_file() && !paths.state_file().is_file() {
        eprintln!(
            "tack: refusing to uninstall: {} has no bin/tack or state.toml, so it is not tack's folder",
            paths.tack.display()
        );
        eprintln!("  fix: unset TACK_HOME, or set it to the folder tack installed into");
        return Ok(2);
    }
    // Undoing tool wiring could strip setup the owner did himself, so uninstall only names it.
    let mut failed = false;
    match State::load(&paths.state_file()) {
        Err(e) => {
            println!("warn: cannot tell which tools tack wired: {e:#}");
            failed = true;
        }
        Ok(s) if !s.wired.is_empty() => {
            let names: Vec<&str> = s.wired.keys().map(String::as_str).collect();
            println!(
                "still wired, left as is: {}; to unwire, run `tack off {}` before uninstalling",
                names.join(", "),
                names.join(" ")
            );
        }
        Ok(_) => {}
    }

    let mut removed = 0;
    for h in Harness::ALL {
        removed += link::prune(&paths.skills_dir(h), &paths.tack, |_| false)?.len();
    }
    claude::remove_hook(paths)?;
    pi::remove_extension(paths)?;
    for h in Harness::ALL {
        rules::set(paths, h, false)?;
    }
    link::remove(&paths.local_bin().join("tack"), &paths.tack)?;
    if paths.tack.exists() {
        fs::remove_dir_all(&paths.tack)
            .with_context(|| format!("removing {}", paths.tack.display()))?;
    }

    println!(
        "removed {removed} skill links, the guard hook and extension, the rules blocks, and {}",
        tilde(&paths.home, &paths.tack)
    );
    println!("pi, T3 Code, Claude Code and other tools stay installed");
    Ok(failed as i32)
}
