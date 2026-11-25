//! Drives the built binary against a temp HOME, a PATH of shim scripts (plus the real git) and a
//! fixture manifest whose skills come from a local git repo made here. Never touches the real home.

use std::fs;
use std::io::Write;
use std::os::unix::fs::{symlink, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

const TACK: &str = env!("CARGO_BIN_EXE_tack");

struct Env {
    _tmp: tempfile::TempDir,
    root: PathBuf,
    home: PathBuf,
    shims: PathBuf,
    manifest: PathBuf,
    log: PathBuf,
}

fn real_git() -> PathBuf {
    let path = std::env::var_os("PATH").expect("PATH");
    std::env::split_paths(&path)
        .map(|d| d.join("git"))
        .find(|p| p.is_file())
        .expect("git on PATH")
}

fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new(real_git())
        .arg("-C")
        .arg(dir)
        .args(args)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .unwrap();
    assert!(out.status.success(), "git {args:?}: {out:?}");
    String::from_utf8(out.stdout).unwrap().trim().to_string()
}

fn shim(dir: &Path, name: &str, body: &str) {
    let p = dir.join(name);
    fs::write(&p, format!("#!/bin/sh\n{body}\n")).unwrap();
    fs::set_permissions(&p, fs::Permissions::from_mode(0o755)).unwrap();
}

fn make_repo(dir: &Path) -> String {
    for name in ["alpha", "beta", "gamma", "delta"] {
        fs::create_dir_all(dir.join(name)).unwrap();
        fs::write(dir.join(name).join("SKILL.md"), format!("# {name}\n")).unwrap();
    }
    git(dir, &["init", "-q"]);
    git(dir, &["add", "."]);
    git(
        dir,
        &[
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@example.com",
            "commit",
            "-q",
            "-m",
            "skills",
        ],
    );
    git(dir, &["rev-parse", "HEAD"])
}

fn fixture(src: &Path, rev: &str) -> String {
    let skill = |name: &str, group: &str, harnesses: &str, default: &str| {
        format!(
            "[[skill]]\nname = \"{name}\"\ndescription = \"The {name} skill\"\nsource = \"{}\"\n\
             path = \"{name}\"\nrev = \"{rev}\"\nharnesses = {harnesses}\ndefault = {default}\n\
             group = \"{group}\"\n\n",
            src.display()
        )
    };
    let both = r#"["claude", "pi"]"#;
    let mut text = String::new();
    text += &skill("alpha", "", both, both);
    text += &skill("beta", "grp", both, both);
    text += &skill("gamma", "grp", r#"["claude"]"#, r#"["claude"]"#);
    text += &skill("delta", "", both, "[]");
    // Off by default and unreachable: install must not try to fetch it.
    text += &skill("epsilon", "", both, "[]").replace(
        &format!("source = \"{}\"", src.display()),
        "source = \"/nonexistent-tack-source/repo\"",
    );
    text += r#"
[[tool]]
name = "pi"
description = "pi"
base = true
required = true
check = ["pi", "--version"]
min_version = "0.84.3"

[[tool.install]]
os = "any"
needs = ["npm"]
run = [["npm", "install", "-g", "pi-fake"]]

[[tool]]
name = "t3"
description = "T3 Code"
base = true
required = true
check = ["t3", "--version"]

[[tool]]
name = "claude"
description = "Claude Code"
base = true
check = ["claude", "--version"]
hint = "Sign in to Claude Code"
manual = "get claude by hand"

[[tool]]
name = "rtk"
description = "Shrinks output"
harnesses = ["claude", "pi"]
default = ["claude", "pi"]
check = ["rtk", "--version"]

[tool.enable]
claude = [["rtk", "init", "-g"]]
pi = [["rtk", "init", "-g", "--agent", "pi"]]

[tool.marker]
pi = "extensions/rtk.ts"

[tool.disable]
claude = [["rtk", "init", "-g", "--uninstall"]]
pi = [["rtk", "init", "--uninstall", "--agent", "pi"]]
"#;
    text
}

impl Env {
    fn new() -> Env {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().canonicalize().unwrap();
        let home = root.join("home");
        let shims = root.join("shims");
        fs::create_dir_all(&home).unwrap();
        fs::create_dir_all(&shims).unwrap();
        let log = root.join("calls.log");
        let logged = |name: &str, reply: &str| {
            format!("echo \"{name} $*\" >> '{}'\necho '{reply}'", log.display())
        };
        shim(
            &shims,
            "git",
            &format!("exec '{}' \"$@\"", real_git().display()),
        );
        shim(&shims, "node", "echo v22.20.0");
        shim(&shims, "npm", &logged("npm", "10.9.0"));
        shim(&shims, "pi", "echo 0.84.3");
        shim(&shims, "t3", "echo t3 1.0.0");
        shim(&shims, "claude", "echo '2.0.0 (Claude Code)'");
        shim(
            &shims,
            "rtk",
            &format!(
                "case \"$*\" in\n\
                 *--uninstall*) ;;\n\
                 *'--agent pi'*) /bin/mkdir -p \"$HOME/.pi/agent/extensions\" && : > \"$HOME/.pi/agent/extensions/rtk.ts\" ;;\n\
                 esac\n{}",
                logged("rtk", "rtk 0.9.0")
            ),
        );

        let src = root.join("src");
        let rev = make_repo(&src);
        let manifest = root.join("manifest.toml");
        fs::write(&manifest, fixture(&src, &rev)).unwrap();
        Env {
            _tmp: tmp,
            root,
            home,
            shims,
            manifest,
            log,
        }
    }

    fn run_with_path(&self, args: &[&str], path: &Path, stdin: &str) -> Output {
        self.run_env(args, path, stdin, &[])
    }

    fn run_env(&self, args: &[&str], path: &Path, stdin: &str, envs: &[(&str, &Path)]) -> Output {
        let mut child = Command::new(TACK)
            .args(args)
            .env_clear()
            .env("HOME", &self.home)
            .env("PATH", path)
            .env("TACK_MANIFEST", &self.manifest)
            .envs(envs.iter().copied())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(stdin.as_bytes())
            .unwrap();
        child.wait_with_output().unwrap()
    }

    fn tack(&self, args: &[&str]) -> Output {
        self.run_with_path(args, &self.shims, "")
    }

    /// Runs tack and asserts exit 0; returns stdout.
    fn ok(&self, args: &[&str]) -> String {
        let out = self.tack(args);
        assert!(
            out.status.success(),
            "tack {args:?} exited {:?}\n{}{}",
            out.status.code(),
            stdout(&out),
            String::from_utf8_lossy(&out.stderr)
        );
        stdout(&out)
    }

    /// A PATH dir holding every shim except `without`.
    fn path_without(&self, without: &str) -> PathBuf {
        let dir = self.root.join(format!("path-without-{without}"));
        fs::create_dir_all(&dir).unwrap();
        for e in fs::read_dir(&self.shims).unwrap().flatten() {
            if e.file_name() != without {
                symlink(e.path(), dir.join(e.file_name())).unwrap();
            }
        }
        dir
    }

    fn link(&self, harness: &str, name: &str) -> PathBuf {
        match harness {
            "claude" => self.home.join(".claude/skills").join(name),
            _ => self.home.join(".pi/agent/skills").join(name),
        }
    }

    fn calls(&self) -> String {
        fs::read_to_string(&self.log).unwrap_or_default()
    }
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn linked(p: &Path) -> bool {
    fs::symlink_metadata(p).is_ok_and(|m| m.file_type().is_symlink())
}

fn exists(p: &Path) -> bool {
    fs::symlink_metadata(p).is_ok()
}

/// Every path under `dir` with its symlink target or file contents, sorted.
fn snapshot(dir: &Path) -> Vec<(PathBuf, String, Vec<u8>)> {
    let mut out = Vec::new();
    let mut todo = vec![dir.to_path_buf()];
    while let Some(d) = todo.pop() {
        for e in fs::read_dir(&d).unwrap().flatten() {
            let p = e.path();
            let ft = e.file_type().unwrap();
            if ft.is_symlink() {
                let target = fs::read_link(&p).unwrap().display().to_string();
                out.push((p, target, Vec::new()));
            } else if ft.is_dir() {
                out.push((p.clone(), "dir".into(), Vec::new()));
                todo.push(p);
            } else {
                let bytes = fs::read(&p).unwrap();
                out.push((p, "file".into(), bytes));
            }
        }
    }
    out.sort();
    out
}

fn row<'a>(list: &'a str, name: &str) -> Option<Vec<&'a str>> {
    list.lines()
        .map(|l| l.split_whitespace().collect::<Vec<_>>())
        .find(|cols| cols.first() == Some(&name))
}

#[test]
fn open_prefers_installed_t3_without_telemetry_or_the_bazzite_banner() {
    let env = Env::new();
    let local_bin = env.home.join(".local/bin");
    fs::create_dir_all(&local_bin).unwrap();
    shim(&env.shims, "t3", "exit 7");
    shim(
        &local_bin,
        "t3",
        r#"test "$T3CODE_TELEMETRY_ENABLED" = false && test "$USERMOTDSOURCED" = Y && test "${PATH%%:*}" = "$HOME/.local/bin""#,
    );
    env.ok(&["open"]);
}

#[test]
fn doctor_reports_each_machine_dependency() {
    let env = Env::new();
    env.ok(&["install"]);
    shim(
        &env.home.join(".tack/bin"),
        "check-dependencies",
        r#"printf 'ok\tfork-example\tlocked\nfail\texample-runtime\tbuild missing\n'; exit 1"#,
    );
    let out = env.tack(&["doctor", "--json"]);
    assert_eq!(out.status.code(), Some(1));
    let report: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let entries = report["checks"].as_array().unwrap();
    for (name, status) in [("fork-example", "ok"), ("example-runtime", "fail")] {
        assert!(entries
            .iter()
            .any(|entry| entry["name"] == name && entry["status"] == status));
    }
}

#[test]
fn managed_open_refuses_an_unverified_t3() {
    let env = Env::new();
    let bin = env.home.join(".tack/bin");
    fs::create_dir_all(&bin).unwrap();
    shim(&bin, "check-dependencies", "exit 1");
    let out = env.tack(&["open"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("pinned T3 cannot start"));
}

#[test]
fn install_links_defaults_and_second_run_changes_nothing() {
    let env = Env::new();
    env.ok(&["install"]);

    for (h, name) in [
        ("claude", "alpha"),
        ("pi", "alpha"),
        ("claude", "beta"),
        ("pi", "beta"),
        ("claude", "gamma"),
    ] {
        let l = env.link(h, name);
        assert!(linked(&l), "{} not linked", l.display());
        assert!(fs::read_link(&l)
            .unwrap()
            .starts_with(env.home.join(".tack/store")));
        assert!(l.join("SKILL.md").is_file());
    }
    for (h, name) in [("pi", "gamma"), ("claude", "delta"), ("pi", "delta")] {
        assert!(!exists(&env.link(h, name)), "{h} {name} should be off");
    }
    assert!(env.home.join(".tack/bin/tack").is_file());
    let t3 = fs::read_to_string(env.home.join(".t3/userdata/settings.json")).unwrap();
    let t3: serde_json::Value = serde_json::from_str(&t3).unwrap();
    assert_eq!(t3["providerInstances"]["codex"]["enabled"], false);
    assert_eq!(t3["providerInstances"]["pi"]["enabled"], true);
    assert_eq!(
        fs::read_link(env.home.join(".local/bin/tack")).unwrap(),
        env.home.join(".tack/bin/tack")
    );
    let calls = env.calls();
    assert!(calls.contains("rtk init -g\n"), "{calls}");
    assert!(calls.contains("rtk init -g --agent pi\n"), "{calls}");
    assert!(
        !calls.contains("npm install"),
        "pi was present, nothing to install"
    );

    let before = snapshot(&env.home);
    let again = env.ok(&["install"]);
    assert!(before == snapshot(&env.home), "second install changed HOME");
    assert!(
        !again.contains(" for "),
        "second install reported changes:\n{again}"
    );
    let calls = env.calls();
    assert_eq!(calls.matches("rtk init -g\n").count(), 1, "{calls}");
    assert_eq!(
        calls.matches("rtk init -g --agent pi\n").count(),
        1,
        "{calls}"
    );

    let home = env.ok(&[]);
    assert!(home.contains("next: tack open"), "{home}");
    assert!(
        home.contains("base: pi 0.84.3, t3 1.0.0, claude 2.0.0"),
        "{home}"
    );
}

#[test]
fn off_then_on_touches_one_harness_only() {
    let env = Env::new();
    env.ok(&["install", "--skip-tools"]);
    assert!(!env.calls().contains("rtk"), "--skip-tools ran a tool step");

    env.ok(&["off", "alpha", "--pi"]);
    assert!(!exists(&env.link("pi", "alpha")));
    assert!(linked(&env.link("claude", "alpha")));
    assert_eq!(
        row(&env.ok(&["list"]), "alpha").unwrap()[1..3],
        ["on", "off"]
    );

    env.ok(&["on", "alpha", "--pi"]);
    assert!(linked(&env.link("pi", "alpha")));
    assert_eq!(
        row(&env.ok(&["list"]), "alpha").unwrap()[1..3],
        ["on", "on"]
    );

    env.ok(&["on", "delta", "--claude"]);
    assert!(linked(&env.link("claude", "delta")));
    assert!(!exists(&env.link("pi", "delta")));
}

#[test]
fn group_toggles_every_member() {
    let env = Env::new();
    env.ok(&["install", "--skip-tools"]);

    let list = env.ok(&["list"]);
    assert_eq!(row(&list, "grp").unwrap()[1..3], ["on", "on"], "{list}");
    assert!(row(&list, "beta").is_none(), "group not collapsed:\n{list}");
    let all = env.ok(&["list", "--all"]);
    assert_eq!(row(&all, "gamma").unwrap()[1..3], ["on", "-"], "{all}");

    env.ok(&["off", "grp"]);
    for (h, name) in [("claude", "beta"), ("pi", "beta"), ("claude", "gamma")] {
        assert!(!exists(&env.link(h, name)), "{h} {name} still linked");
    }
    assert!(linked(&env.link("claude", "alpha")));

    env.ok(&["on", "grp", "--claude"]);
    assert!(linked(&env.link("claude", "beta")));
    assert!(linked(&env.link("claude", "gamma")));
    assert!(!exists(&env.link("pi", "beta")));
    assert_eq!(row(&env.ok(&["list"]), "grp").unwrap()[1..3], ["on", "off"]);

    assert_eq!(env.tack(&["on", "nope"]).status.code(), Some(2));
    assert_eq!(env.tack(&["on", "gamma", "--pi"]).status.code(), Some(2));
    let out = env.tack(&["off", "pi"]);
    assert_eq!(out.status.code(), Some(2));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("pi is a base tool; it is always on"), "{err}");
}

#[test]
fn foreign_skill_is_left_alone_and_survives_uninstall() {
    let env = Env::new();
    let mine = env.link("claude", "alpha");
    fs::create_dir_all(&mine).unwrap();
    fs::write(mine.join("SKILL.md"), "mine").unwrap();

    let out = env.ok(&["install"]);
    assert!(out.contains("exists (not tack's)"), "{out}");
    assert!(!linked(&mine));
    assert_eq!(fs::read_to_string(mine.join("SKILL.md")).unwrap(), "mine");
    assert!(linked(&env.link("pi", "alpha")));
    assert_eq!(
        row(&env.ok(&["list"]), "alpha").unwrap()[1..3],
        ["exists", "on"]
    );
    assert!(stdout(&env.tack(&["doctor"])).contains("exists (not tack's)"));

    let out = env.ok(&["uninstall"]);
    assert!(out.contains("stay installed"), "{out}");
    assert_eq!(fs::read_to_string(mine.join("SKILL.md")).unwrap(), "mine");
    assert!(!exists(&env.home.join(".tack")));
    assert!(!exists(&env.home.join(".local/bin/tack")));
    let tack_home = env.home.join(".tack");
    let leftover: Vec<_> = snapshot(&env.home)
        .into_iter()
        .filter(|(_, target, _)| Path::new(target).starts_with(&tack_home))
        .collect();
    assert!(leftover.is_empty(), "tack links left: {leftover:?}");
    assert!(out.contains("tack off rtk"), "{out}");
    let calls = env.calls();
    assert!(
        !calls.contains("--uninstall"),
        "uninstall ran a disable step:\n{calls}"
    );
}

#[test]
fn off_runs_a_tools_disable_step_for_that_harness() {
    let env = Env::new();
    env.ok(&["install"]);
    env.ok(&["off", "rtk", "--claude"]);
    let calls = env.calls();
    assert!(calls.contains("rtk init -g --uninstall\n"), "{calls}");
    assert!(!calls.contains("--uninstall --agent pi"), "{calls}");
    assert_eq!(row(&env.ok(&["list"]), "rtk").unwrap()[1..3], ["off", "on"]);
}

#[test]
fn off_runs_no_disable_step_for_a_tool_tack_never_wired() {
    let env = Env::new();
    env.ok(&["install", "--skip-tools"]);
    env.ok(&["off", "rtk"]);
    assert!(!env.calls().contains("--uninstall"), "{}", env.calls());
    assert_eq!(
        row(&env.ok(&["list"]), "rtk").unwrap()[1..3],
        ["off", "off"]
    );
}

#[test]
fn install_keeps_choices_and_skips_sources_that_are_off() {
    let env = Env::new();
    env.ok(&["install", "--skip-tools"]);
    env.ok(&["off", "alpha"]);
    env.ok(&["install", "--skip-tools"]);
    assert_eq!(
        row(&env.ok(&["list"]), "alpha").unwrap()[1..3],
        ["off", "off"]
    );
    assert!(!exists(&env.link("claude", "alpha")));
    // epsilon's source does not exist; it is off, so install never fetched it.
    let out = env.tack(&["on", "epsilon"]);
    assert_eq!(out.status.code(), Some(1), "{}", stdout(&out));
}

#[test]
fn install_removes_stale_links() {
    let env = Env::new();
    env.ok(&["install", "--skip-tools"]);
    let gone = env.home.join(".tack/store/own/gone");
    for h in ["claude", "pi"] {
        symlink(&gone, env.link(h, "gone")).unwrap();
    }
    let mine = env.link("claude", "mine");
    symlink(env.root.join("elsewhere"), &mine).unwrap();
    let text = fs::read_to_string(&env.manifest).unwrap();
    let without_alpha: Vec<&str> = text
        .split("\n\n")
        .filter(|block| !block.contains("name = \"alpha\""))
        .collect();
    fs::write(&env.manifest, without_alpha.join("\n\n")).unwrap();

    env.ok(&["install", "--skip-tools"]);
    for h in ["claude", "pi"] {
        assert!(!exists(&env.link(h, "gone")), "{h} gone still linked");
        assert!(!exists(&env.link(h, "alpha")), "{h} alpha still linked");
        assert!(linked(&env.link(h, "beta")));
    }
    assert!(linked(&mine), "a link that is not tack's was removed");
}

#[test]
fn list_before_install_says_so() {
    let env = Env::new();
    let first = env.ok(&["list"]).lines().next().unwrap().to_string();
    assert_eq!(
        first,
        "not installed yet; this is what tack install will turn on"
    );
    env.ok(&["install", "--skip-tools"]);
    assert!(!env.ok(&["list"]).contains("not installed yet"));
}

#[test]
fn broken_state_gives_a_fix_and_doctor_and_uninstall_still_run() {
    let env = Env::new();
    env.ok(&["install", "--skip-tools"]);
    let state = env.home.join(".tack/state.toml");
    fs::write(&state, "[enabled.alpha\nclaude = yes\n").unwrap();

    let out = env.tack(&["list"]);
    assert_eq!(out.status.code(), Some(1));
    let err = String::from_utf8_lossy(&out.stderr);
    let fix = format!("fix: mv {0} {0}.bak && tack install", state.display());
    assert!(err.contains(&fix), "{err}");

    let out = env.tack(&["doctor"]);
    assert_eq!(out.status.code(), Some(1));
    let text = stdout(&out);
    assert!(
        text.contains("fail: state:") && text.contains(&fix),
        "{text}"
    );

    env.tack(&["uninstall"]);
    assert!(!exists(&env.home.join(".tack")));
    assert!(!exists(&env.link("claude", "beta")));
}

#[test]
fn uninstall_refuses_a_folder_that_is_not_tacks() {
    let env = Env::new();
    let docs = env.home.join("Documents");
    fs::create_dir_all(&docs).unwrap();
    fs::write(docs.join("notes.txt"), "keep").unwrap();
    let out = env.run_env(&["uninstall"], &env.shims, "", &[("TACK_HOME", &docs)]);
    assert_eq!(out.status.code(), Some(2));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("refusing"), "{err}");
    assert!(docs.join("notes.txt").is_file());
}

#[test]
fn doctor_passes_then_fails_with_a_fix_when_a_required_tool_is_missing() {
    let env = Env::new();
    env.ok(&["install", "--skip-tools"]);
    // The guard stream owns the hook; switch it off so this test does not depend on it.
    env.ok(&["off", "guard"]);
    env.ok(&["doctor"]);

    fs::remove_file(env.shims.join("claude")).unwrap();
    let text = stdout(&env.tack(&["doctor"]));
    assert!(text.contains("fix: get claude by hand"), "{text}");

    fs::remove_file(env.shims.join("pi")).unwrap();
    let out = env.tack(&["doctor"]);
    assert_eq!(out.status.code(), Some(1));
    let text = stdout(&out);
    assert!(text.contains("fail: pi: pi missing"), "{text}");
    assert!(text.contains("fix: tack install"), "{text}");

    let json: serde_json::Value =
        serde_json::from_slice(&env.tack(&["doctor", "--json"]).stdout).unwrap();
    assert_eq!(json["ok"], false);

    // npm "installs" pi but the shim never appears, so install must fail.
    let out = env.tack(&["install"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(env.calls().contains("npm install -g pi-fake"));
}

#[test]
fn missing_prerequisites_exit_2_with_a_fix() {
    let env = Env::new();
    for missing in ["git", "node", "npm"] {
        let out = env.run_with_path(&["install"], &env.path_without(missing), "");
        assert_eq!(out.status.code(), Some(2), "without {missing}");
        let err = String::from_utf8_lossy(&out.stderr);
        assert!(err.contains(missing) && err.contains("fix: "), "{err}");
    }
    shim(&env.shims, "node", "echo v22.18.9");
    let out = env.tack(&["install"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(!exists(&env.home.join(".tack")));
}

#[test]
fn guard_denies_unparseable_input() {
    let env = Env::new();
    let out = env.run_with_path(&["guard", "--harness", "claude"], &env.shims, "not json");
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("Blocked by tack guard"));
}

#[test]
fn install_rewires_a_tool_whose_marker_file_is_gone() {
    let env = Env::new();
    env.ok(&["install"]);
    let marker = env.home.join(".pi/agent/extensions/rtk.ts");
    assert!(marker.is_file());

    fs::remove_file(&marker).unwrap();
    let out = stdout(&env.tack(&["doctor"]));
    assert!(out.contains("warn: rtk (pi)"), "{out}");
    assert!(out.contains("fix: tack install"), "{out}");

    env.ok(&["install"]);
    assert!(marker.is_file());
    assert_eq!(
        env.calls().matches("rtk init -g --agent pi\n").count(),
        2,
        "{}",
        env.calls()
    );
    assert_eq!(env.calls().matches("rtk init -g\n").count(), 1);
    assert!(!stdout(&env.tack(&["doctor"])).contains("rtk (pi)"));
}

#[test]
fn rules_block_goes_in_both_agents_and_comes_out_cleanly() {
    let env = Env::new();
    let claude_md = env.home.join(".claude/CLAUDE.md");
    let agents_md = env.home.join(".pi/agent/AGENTS.md");
    fs::create_dir_all(claude_md.parent().unwrap()).unwrap();
    fs::write(&claude_md, "# Mine\n").unwrap();
    let block = "<!-- tack:rules:start -->\n";

    env.ok(&["install", "--skip-tools"]);
    assert!(fs::read_to_string(&claude_md)
        .unwrap()
        .starts_with(&format!("# Mine\n\n{block}")));
    assert!(fs::read_to_string(&agents_md).unwrap().starts_with(block));
    let ext = env.home.join(".pi/agent/extensions/tack-rules.ts");
    let guard = env.home.join(".pi/agent/extensions/tack-guard.ts");
    assert!(fs::read_to_string(&ext)
        .unwrap()
        .starts_with("// tack-rules: written by tack."));
    assert!(!exists(&env.home.join(".claude/extensions")));

    env.ok(&["off", "rules", "--pi"]);
    assert!(!exists(&agents_md));
    assert!(!exists(&ext));
    assert!(exists(&guard));
    assert!(fs::read_to_string(&claude_md).unwrap().contains(block));

    env.ok(&["on", "rules", "--pi"]);
    assert!(exists(&ext));
    env.ok(&["uninstall"]);
    assert_eq!(fs::read_to_string(&claude_md).unwrap(), "# Mine\n");
    assert!(!exists(&agents_md));
    assert!(!exists(&ext));
}
