//! The guard: a pattern-matching deny list for agent tool calls. It is not a sandbox.
//!
//! Denied:
//! - file tools that name a credential path or an env file (`.env`, `.env.*` except the
//!   example/sample/template names); Glob `pattern` and Grep `glob` resolve against the tool's
//!   `path`. On macOS these names compare case-insensitively.
//! - file tools other than read/list/search that name anything under `$TACK_HOME` (else `~/.tack`)
//!   or the pi extension `~/.pi/agent/extensions/tack-guard.ts`.
//! - shell commands that name any of those paths (except running the tack binary itself),
//!   read the macOS keychain, force-push (`--force`, `-f`, `--force-with-lease`, a `+refspec`),
//!   or run `rm -r` on `/`, `~` or `$HOME`. Leading `VAR=x` and wrappers (`sudo`, `env`, `timeout`,
//!   `nice`, `xargs`, `rtk`, ...) with their flags are skipped; `eval`, `sh -c`, `env -S`, heredoc bodies
//!   (whoever reads them), `((...))`, and `$(...)` / backticks (also inside double quotes and
//!   unquoted heredocs) are checked as commands, nested at most 8 deep. `#` at a word start
//!   comments out the rest of the line.
//! - Env-file names (only those) are fine as text: `echo`/`printf` arguments (unless a later
//!   segment of the pipeline runs `xargs` or a shell), heredoc bodies not fed to a shell, grep/rg
//!   patterns, `git rm --cached`, `git check-ignore`, `git ls-files`. Redirect targets are always
//!   paths.
//!
//! Out of reach: paths built at run time (`cat $DIR/id_rsa`, `cd ~; cat .ssh/id_rsa`), variables
//! other than `$HOME` and `$TACK_HOME`, globs that expand to a protected path (`~/.s*/id_rsa`),
//! `eval` of computed strings, encoded or obfuscated strings, scripts written to disk and run
//! later, interpreters (`python -c`, `node -e`), removing a parent of a protected path
//! (`rm -rf ~/.pi`), and any program that reads a secret by itself. `~/.claude/settings.json` is
//! not protected (the owner edits it through the agent), so an agent can remove the Claude hook.
//! It only inspects the words of one tool call.
use std::iter::Peekable;
use std::path::{Component, Path, PathBuf};
use std::str::Chars;

use serde_json::Value;

use crate::paths::Harness;

/// Credential paths, relative to `$HOME`. Each matches itself and everything under it.
const CREDENTIAL_PATHS: &[&str] = &[
    ".ssh",
    ".aws",
    ".gnupg",
    ".config/gh",
    ".netrc",
    ".npmrc",
    ".docker/config.json",
    ".kube/config",
    ".claude/.credentials.json",
    ".pi/agent/auth.json",
    ".codex/auth.json",
];

const ENV_ALLOWED: &[&str] = &[".env.example", ".env.sample", ".env.template"];

/// Tools that only read, list or search; any other tool may write to the paths it names.
const READ_TOOLS: &[&str] = &["read", "glob", "grep", "ls", "find", "notebookread"];

const KEYCHAIN_READS: &[&str] = &[
    "find-generic-password",
    "find-internet-password",
    "dump-keychain",
];

/// Leading words that run the words after them as the command, with their short and long flags
/// that take a separate value.
const WRAPPERS: &[(&str, &str, &[&str])] = &[
    (
        "sudo",
        "CDghpRrTtUu",
        &["--user", "--group", "--host", "--prompt", "--chdir"],
    ),
    ("doas", "Cu", &[]),
    ("env", "CSu", &["--split-string", "--unset", "--chdir"]),
    ("command", "", &[]),
    ("exec", "a", &[]),
    ("nohup", "", &[]),
    ("time", "", &[]),
    ("nice", "n", &[]),
    ("timeout", "ks", &[]),
    ("xargs", "adEILnPs", &[]),
    ("rtk", "c", &["--shell", "--command"]),
];

const GIT_OPTS_WITH_VALUE: &[&str] = &["-C", "-c", "--git-dir", "--work-tree", "--namespace"];

const SHELLS: &[&str] = &["sh", "bash", "zsh", "dash"];

const MAX_DEPTH: usize = 8;

/// Returns (exit code, stderr text). 0 = allow, 2 = deny.
pub fn run(harness: Harness, stdin: &str, home: &Path) -> (i32, String) {
    let tack = std::env::var_os("TACK_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".tack"));
    judge(harness, stdin, home, &tack)
}

fn judge(harness: Harness, stdin: &str, home: &Path, tack: &Path) -> (i32, String) {
    let reason = match parse(harness, stdin) {
        None => Some("could not read the tool call".to_string()),
        Some(call) => {
            let cwd = call
                .cwd
                .or_else(|| std::env::current_dir().ok())
                .unwrap_or_else(|| PathBuf::from("/"));
            let ctx = Ctx {
                home: lexical(home),
                cwd: lexical(&cwd),
                tack: lexical(tack),
                env_text: false,
            };
            check_input(&call.tool, &call.input, &ctx)
        }
    };
    match reason {
        None => (0, String::new()),
        Some(r) => (2, format!("Blocked by tack guard: {r}")),
    }
}

struct Call {
    tool: String,
    input: Value,
    cwd: Option<PathBuf>,
}

/// Claude Code sends `{tool_name, tool_input, cwd, ...}`; the pi extension sends `{tool, input, cwd}`.
fn parse(harness: Harness, stdin: &str) -> Option<Call> {
    let v: Value = serde_json::from_str(stdin).ok()?;
    let (tool_key, input_key) = match harness {
        Harness::Claude => ("tool_name", "tool_input"),
        Harness::Pi => ("tool", "input"),
    };
    Some(Call {
        tool: v.get(tool_key)?.as_str()?.to_string(),
        input: v.get(input_key).filter(|i| i.is_object())?.clone(),
        cwd: v.get("cwd").and_then(Value::as_str).map(PathBuf::from),
    })
}

#[derive(Clone)]
struct Ctx {
    home: PathBuf,
    cwd: PathBuf,
    tack: PathBuf,
    /// Inside heredoc text: env-file names are text.
    env_text: bool,
}

fn check_input(tool: &str, input: &Value, ctx: &Ctx) -> Option<String> {
    let tool = tool.to_ascii_lowercase();
    let field = |k: &str| input.get(k).and_then(Value::as_str);
    // Glob patterns are relative to the tool's search root, not the cwd.
    let in_root = Ctx {
        cwd: field("path").map_or_else(|| ctx.cwd.clone(), |p| normalize(p, ctx)),
        ..ctx.clone()
    };
    let mut paths: Vec<(&str, &Ctx)> = ["file_path", "path", "notebook_path"]
        .iter()
        .filter_map(|k| Some((field(k)?, ctx)))
        .collect();
    // A grep pattern is a regex, not a path: searching code for ".env" is everyday work.
    if tool != "grep" {
        paths.extend(field("pattern").map(|p| (p, &in_root)));
    }
    paths.extend(field("glob").map(|p| (p, &in_root)));
    let writes = !READ_TOOLS.contains(&tool.as_str());
    paths
        .into_iter()
        .find_map(|(p, c)| {
            check_path(p, c, false).or_else(|| writes.then(|| check_tack(p, c)).flatten())
        })
        .or_else(|| field("command").and_then(|c| check_command(c, ctx, 0)))
}

fn check_path(raw: &str, ctx: &Ctx, env_ok: bool) -> Option<String> {
    let path = normalize(raw, ctx);
    if let Some(cred) = CREDENTIAL_PATHS
        .iter()
        .find(|c| under(&path, &ctx.home.join(c)))
    {
        return Some(format!("touches credential path ~/{cred}"));
    }
    if env_ok || ctx.env_text {
        return None;
    }
    let name = path.file_name()?.to_str()?;
    let folded = fold(name);
    ((folded == ".env" || folded.starts_with(".env.")) && !ENV_ALLOWED.contains(&folded.as_str()))
        .then(|| format!("touches env file {name}"))
}

fn check_tack(raw: &str, ctx: &Ctx) -> Option<String> {
    let path = normalize(raw, ctx);
    let pi_ext = ctx.home.join(".pi/agent/extensions/tack-guard.ts");
    (under(&path, &ctx.tack) || under(&path, &pi_ext)).then(|| "touches tack's own files".into())
}

/// macOS file systems ignore case by default, so there `~/.SSH` is `~/.ssh`.
fn fold(s: &str) -> String {
    if cfg!(target_os = "macos") {
        s.to_lowercase()
    } else {
        s.to_string()
    }
}

fn under(path: &Path, base: &Path) -> bool {
    Path::new(&fold(&path.to_string_lossy())).starts_with(fold(&base.to_string_lossy()))
}

/// Expands `~`, `$HOME`, `$TACK_HOME` (and the `${...}` forms), makes the path absolute against
/// the cwd, then resolves it.
fn normalize(raw: &str, ctx: &Ctx) -> PathBuf {
    let home = ctx.home.to_string_lossy();
    let tack = ctx.tack.to_string_lossy();
    let mut s = raw
        .replace("${TACK_HOME}", &tack)
        .replace("$TACK_HOME", &tack)
        .replace("${HOME}", &home)
        .replace("$HOME", &home);
    if s == "~" || s.starts_with("~/") {
        s = format!("{home}{}", &s[1..]);
    }
    lexical(&ctx.cwd.join(s))
}

/// Resolves `.` and `..` without touching the filesystem.
fn lexical(p: &Path) -> PathBuf {
    let mut out = PathBuf::from("/");
    for c in p.components() {
        match c {
            Component::ParentDir => {
                out.pop();
            }
            Component::Normal(n) => out.push(n),
            _ => {}
        }
    }
    out
}

/// The program a word names. macOS finds `GIT` as `git`, so the name is folded like a path.
fn prog(word: &str) -> String {
    fold(word.rsplit('/').next().unwrap_or(word))
}

fn check_command(cmd: &str, ctx: &Ctx, depth: usize) -> Option<String> {
    if depth > MAX_DEPTH {
        return Some(format!("nests commands more than {MAX_DEPTH} deep"));
    }
    let p = split_command(cmd);
    let shell_reads_stdin = p.segs.iter().any(|s| {
        command_words(&s.words)
            .first()
            .is_some_and(|w| SHELLS.contains(&prog(w).as_str()))
    });
    // Heredoc text is checked as commands whoever reads it, so a misparse never hides one; its
    // env-file names stay text unless a shell runs it.
    let text = Ctx {
        env_text: ctx.env_text || !shell_reads_stdin,
        ..ctx.clone()
    };
    (0..p.segs.len())
        .find_map(|i| check_seg(&p.segs[i], ctx, depth, feeds_exec(&p.segs[i..])))
        // Heredoc text may still feed paths to `xargs` and the like; only env-file names pass.
        .or_else(|| {
            p.stdin
                .iter()
                .flat_map(|b| b.split_whitespace())
                .find_map(|w| check_word(w, ctx, true))
        })
        .or_else(|| p.subs.iter().find_map(|c| check_command(c, ctx, depth + 1)))
        .or_else(|| {
            p.stdin
                .iter()
                .find_map(|b| check_command(b, &text, depth + 1))
        })
}

/// Whether a later segment of the same pipeline runs its input: `xargs` or a shell.
fn feeds_exec(segs: &[Seg]) -> bool {
    segs.iter()
        .zip(&segs[1..])
        .take_while(|(a, _)| a.piped)
        .any(|(_, b)| {
            let n = b.words.len() - command_words(&b.words).len();
            b.words
                .iter()
                .take(n + 1)
                .any(|w| prog(w) == "xargs" || SHELLS.contains(&prog(w).as_str()))
        })
}

fn check_seg(seg: &Seg, ctx: &Ctx, depth: usize, feeds_exec: bool) -> Option<String> {
    let words = command_words(&seg.words);
    let prefix = &seg.words[..seg.words.len() - words.len()];
    prefix
        .iter()
        .chain(&seg.redirects)
        .find_map(|w| check_word(w, ctx, false))
        .or_else(|| env_split(prefix).and_then(|c| check_command(c, ctx, depth + 1)))
        .or_else(|| check_args(words, ctx, feeds_exec))
        .or_else(|| check_segment(words, ctx, depth))
        .or_else(|| check_rtk(prefix, words, ctx, depth))
}

/// rtk subcommands that run the command after them; `command_words` skips them like wrappers.
/// `run`, `err`, `summary` and `test` can also hand one string to a shell.
const RTK_RUNNERS: &[&str] = &["proxy", "bypass", "run", "err", "summary", "test"];

/// Extra checks when rtk wraps the command. Every tail of the words is checked as a command (an
/// unknown subcommand may run them). rtk's shell strings are checked as command lines: the value
/// of `-c` / `--command` / `--flag=value`, and after a runner a lone word (`rtk run 'cat<x'` goes
/// through `sh -c`). Text arguments of the wrapped command are left alone.
fn check_rtk(prefix: &[String], words: &[String], ctx: &Ctx, depth: usize) -> Option<String> {
    let r = prefix.iter().position(|w| prog(w) == "rtk")?;
    let opts = &prefix[r + 1..];
    let runner = opts.iter().any(|w| RTK_RUNNERS.contains(&w.as_str()));
    let strings = opts
        .iter()
        .enumerate()
        .filter_map(|(i, w)| match w.as_str() {
            "-c" | "--command" => opts.get(i + 1).map(String::as_str),
            _ if w.starts_with("-c") && !w.contains('=') => Some(&w[2..]),
            _ => w
                .split_once('=')
                .filter(|_| w.starts_with('-'))
                .map(|(_, v)| v),
        });
    let lone = words
        .first()
        .filter(|_| runner && words.len() == 1)
        .map(String::as_str);
    (1..words.len())
        .find_map(|i| check_segment(&words[i..], ctx, depth))
        .or_else(|| {
            strings
                .chain(lone)
                .find_map(|c| check_command(c, ctx, depth + 1))
        })
}

/// The value of env's `-S` / `--split-string`: a command line of its own.
fn env_split(prefix: &[String]) -> Option<&str> {
    let e = prefix.iter().position(|w| prog(w) == "env")?;
    let rest = &prefix[e + 1..];
    rest.iter().enumerate().find_map(|(i, w)| match w.as_str() {
        "-S" | "--split-string" => rest.get(i + 1).map(String::as_str),
        _ => w
            .strip_prefix("--split-string=")
            .or_else(|| w.strip_prefix("-S"))
            .filter(|v| !v.is_empty()),
    })
}

/// Checks a word, and the value of a `name=value` word, as a path.
fn check_word(w: &str, ctx: &Ctx, env_ok: bool) -> Option<String> {
    let value = w.split_once('=').map(|(_, v)| v);
    std::iter::once(w)
        .chain(value)
        .find_map(|p| check_path(p, ctx, env_ok).or_else(|| check_tack(p, ctx)))
}

/// Path checks on the command word and its arguments; some arguments are text, not files.
fn check_args(words: &[String], ctx: &Ctx, feeds_exec: bool) -> Option<String> {
    let (first, args) = words.split_first()?;
    let hit = if normalize(first, ctx) == ctx.tack.join("bin/tack") {
        check_path(first, ctx, false)
    } else {
        check_word(first, ctx, false)
    };
    if hit.is_some() {
        return hit;
    }
    let env_ok: Vec<bool> = match prog(first).as_str() {
        "echo" | "printf" => vec![!feeds_exec; args.len()],
        "grep" | "rg" => {
            let pattern = (0..args.len()).find(|&i| {
                !args[i].starts_with('-')
                    && (i == 0 || !matches!(args[i - 1].as_str(), "-f" | "--file"))
            });
            (0..args.len()).map(|i| Some(i) == pattern).collect()
        }
        "git" => {
            let (sub, rest) = git_sub(args);
            let ok = matches!(sub, Some("check-ignore" | "ls-files"))
                || (sub == Some("rm") && rest.iter().any(|w| w == "--cached"));
            vec![ok; args.len()]
        }
        _ => vec![false; args.len()],
    };
    args.iter()
        .zip(env_ok)
        .find_map(|(w, ok)| check_word(w, ctx, ok))
}

/// Drops leading `VAR=value` assignments and wrappers such as `sudo -E`, with their flags.
fn command_words(seg: &[String]) -> &[String] {
    let mut i = 0;
    while let Some(w) = seg.get(i) {
        if w.contains('=') && !w.starts_with(['-', '=']) {
            i += 1;
            continue;
        }
        let Some((name, takes, long)) = WRAPPERS.iter().find(|(n, ..)| *n == prog(w)) else {
            break;
        };
        i = skip_flags(seg, i + 1, takes, long);
        if *name == "timeout" {
            i += 1;
        }
        let runner = |w: &String| RTK_RUNNERS.contains(&w.as_str());
        if *name == "rtk" && seg.get(i).is_some_and(runner) {
            i = skip_flags(seg, i + 1, takes, long);
        }
    }
    &seg[i.min(seg.len())..]
}

/// The index after the flags at `i`, with the separate values of `takes` / `long` flags.
fn skip_flags(seg: &[String], mut i: usize, takes: &str, long: &[&str]) -> usize {
    while let Some(f) = seg.get(i).filter(|f| f.starts_with('-')) {
        i += 1;
        if f == "--" {
            break;
        }
        if (f.len() == 2 && takes.contains(&f[1..])) || long.contains(&f.as_str()) {
            i += 1;
        }
    }
    i
}

/// The git subcommand and the words after it, skipping global options and their values.
fn git_sub(args: &[String]) -> (Option<&str>, &[String]) {
    let mut i = 0;
    while let Some(w) = args.get(i) {
        i += 1;
        if GIT_OPTS_WITH_VALUE.contains(&w.as_str()) {
            i += 1;
        } else if !w.starts_with('-') {
            return (Some(w), &args[i..]);
        }
    }
    (None, &[])
}

fn short_flag(w: &str, c: char) -> bool {
    w.starts_with('-') && !w.starts_with("--") && w.contains(c)
}

fn check_segment(words: &[String], ctx: &Ctx, depth: usize) -> Option<String> {
    let (first, args) = words.split_first()?;
    match prog(first).as_str() {
        "security" if args.iter().any(|a| KEYCHAIN_READS.contains(&a.as_str())) => {
            Some("reads the macOS keychain".into())
        }
        "git" => {
            let (sub, rest) = git_sub(args);
            let force = rest.iter().any(|w| {
                w == "--force"
                    || w.starts_with("--force-with-lease")
                    || short_flag(w, 'f')
                    || w.starts_with('+')
            });
            (sub == Some("push") && force).then(|| "force-pushes".into())
        }
        "rm" => {
            let recursive = args
                .iter()
                .any(|w| w == "--recursive" || short_flag(w, 'r') || short_flag(w, 'R'));
            let wipes = args.iter().filter(|w| !w.starts_with('-')).any(|w| {
                let p = normalize(w.trim_end_matches('*'), ctx);
                p == Path::new("/") || p == ctx.home
            });
            (recursive && wipes).then(|| "rm -r on / or the home directory".into())
        }
        "eval" => check_command(&args.join(" "), ctx, depth + 1),
        p if SHELLS.contains(&p) => {
            let c = args.iter().position(|w| short_flag(w, 'c'))?;
            check_command(args.get(c + 1)?, ctx, depth + 1)
        }
        _ => None,
    }
}

#[derive(Default, Debug, PartialEq)]
struct Seg {
    words: Vec<String>,
    /// Targets of `<`, `>`, `>>`: always paths, whatever the command is.
    redirects: Vec<String>,
    /// Output goes through `|` to the next segment.
    piped: bool,
}

#[derive(Default)]
struct Parsed {
    segs: Vec<Seg>,
    /// `$(...)` and backtick commands inside double quotes or an unquoted heredoc.
    subs: Vec<String>,
    /// Heredoc and here-string bodies: plain text unless a shell reads them.
    stdin: Vec<String>,
}

/// What the next finished word is.
#[derive(Clone, Copy, Default)]
enum Next {
    #[default]
    Word,
    Redirect,
    HereString,
    Delimiter {
        strip_tabs: bool,
    },
}

#[derive(Default)]
struct Splitter {
    p: Parsed,
    seg: Seg,
    word: Option<String>,
    /// The current word had a quote or backslash (a quoted heredoc delimiter disables expansion).
    quoted: bool,
    next: Next,
    /// Delimiter, quoted, strip tabs: bodies start after the next newline.
    heredocs: Vec<(String, bool, bool)>,
}

impl Splitter {
    fn word(&mut self) -> &mut String {
        self.word.get_or_insert_with(String::new)
    }

    fn flush(&mut self) {
        if let Some(w) = self.word.take() {
            match self.next {
                Next::Word => self.seg.words.push(w),
                Next::Redirect => self.seg.redirects.push(w),
                Next::HereString => self.p.stdin.push(w),
                Next::Delimiter { strip_tabs } => self.heredocs.push((w, self.quoted, strip_tabs)),
            }
            self.next = Next::Word;
            self.quoted = false;
        }
    }

    fn end_segment(&mut self) {
        self.flush();
        self.next = Next::Word;
        if !self.seg.words.is_empty() || !self.seg.redirects.is_empty() {
            self.p.segs.push(std::mem::take(&mut self.seg));
        }
    }
}

/// Splits a shell command into segments (at `;`, `&`, `|`, newlines, parentheses and backticks),
/// each a list of words plus redirect targets. Quotes group words; nothing is expanded.
fn split_command(cmd: &str) -> Parsed {
    let mut s = Splitter::default();
    let mut chars = cmd.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\'' => {
                s.quoted = true;
                s.word().extend(chars.by_ref().take_while(|&c| c != '\''));
            }
            '"' => {
                s.quoted = true;
                s.word();
                while let Some(c) = chars.next() {
                    match c {
                        '"' => break,
                        '\\' => {
                            if let Some(n) = chars.next() {
                                if !"$`\"\\\n".contains(n) {
                                    s.word().push('\\');
                                }
                                s.word().push(n);
                            }
                        }
                        '$' if chars.next_if_eq(&'(').is_some() => {
                            let sub = take_paren(&mut chars);
                            s.word().push_str(&format!("$({sub})"));
                            s.p.subs.push(sub);
                        }
                        '`' => {
                            let sub = take_backtick(&mut chars);
                            s.word().push_str(&format!("`{sub}`"));
                            s.p.subs.push(sub);
                        }
                        _ => s.word().push(c),
                    }
                }
            }
            '\\' => {
                s.quoted = true;
                s.word().extend(chars.next());
            }
            '\n' => {
                s.end_segment();
                for (delim, quoted, strip_tabs) in std::mem::take(&mut s.heredocs) {
                    let body = take_heredoc(&mut chars, &delim, strip_tabs);
                    if !quoted {
                        substitutions(&body, &mut s.p.subs);
                    }
                    s.p.stdin.push(body);
                }
            }
            '#' if s.word.is_none() => while chars.next_if(|&c| c != '\n').is_some() {},
            '(' if chars.next_if_eq(&'(').is_some() => {
                // `((...))` / `$((...))` is arithmetic, where `<<` is a shift. Bash may still
                // read `((a) )` as nested subshells, so the inside is checked as a command.
                let inner = take_paren(&mut chars);
                chars.next_if_eq(&')');
                s.word().push_str(&format!("(({inner}))"));
                s.p.subs.push(inner);
            }
            '|' => {
                let piped = chars.next_if_eq(&'|').is_none();
                s.end_segment();
                if let Some(last) = s.p.segs.last_mut() {
                    last.piped = piped;
                }
            }
            ';' | '&' | '(' | ')' | '`' => s.end_segment(),
            '<' | '>' => {
                s.flush();
                s.next = if c == '<' && chars.next_if_eq(&'<').is_some() {
                    if chars.next_if_eq(&'<').is_some() {
                        Next::HereString
                    } else {
                        Next::Delimiter {
                            strip_tabs: chars.next_if_eq(&'-').is_some(),
                        }
                    }
                } else {
                    // `>>`, `>&`, `>|`, `<&`, `<>`
                    chars.next_if(|n| matches!(n, '>' | '&' | '|'));
                    Next::Redirect
                };
            }
            c if c.is_whitespace() => s.flush(),
            c => s.word().push(c),
        }
    }
    s.end_segment();
    s.p
}

/// Reads up to the `)` that closes a `$(` already consumed.
fn take_paren(chars: &mut Peekable<Chars>) -> String {
    let mut s = String::new();
    let mut depth = 1;
    while let Some(c) = chars.next() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
            '\\' => {
                s.push(c);
                s.extend(chars.next());
                continue;
            }
            '\'' | '"' => {
                s.push(c);
                while let Some(n) = chars.next() {
                    s.push(n);
                    if n == c {
                        break;
                    }
                    if n == '\\' && c == '"' {
                        s.extend(chars.next());
                    }
                }
                continue;
            }
            _ => {}
        }
        s.push(c);
    }
    s
}

/// Reads up to the closing backtick.
fn take_backtick(chars: &mut Peekable<Chars>) -> String {
    let mut s = String::new();
    while let Some(c) = chars.next() {
        match c {
            '`' => break,
            '\\' => s.extend(chars.next()),
            _ => s.push(c),
        }
    }
    s
}

/// Reads heredoc lines up to the delimiter line.
fn take_heredoc(chars: &mut Peekable<Chars>, delim: &str, strip_tabs: bool) -> String {
    let mut body = String::new();
    while chars.peek().is_some() {
        let line: String = chars.by_ref().take_while(|&c| c != '\n').collect();
        let bare = if strip_tabs {
            line.trim_start_matches('\t')
        } else {
            &line
        };
        if bare == delim {
            break;
        }
        body.push_str(&line);
        body.push('\n');
    }
    body
}

/// Collects `$(...)` and backtick commands from text the shell expands, such as a heredoc body.
fn substitutions(text: &str, out: &mut Vec<String>) {
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' => {
                chars.next();
            }
            '$' if chars.next_if_eq(&'(').is_some() => out.push(take_paren(&mut chars)),
            '`' => out.push(take_backtick(&mut chars)),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const HOME: &str = "/home/u";

    fn claude(tool: &str, input: &Value) -> String {
        json!({"tool_name": tool, "tool_input": input, "cwd": "/work/proj"}).to_string()
    }

    fn pi(tool: &str, input: &Value) -> String {
        json!({"tool": tool.to_lowercase(), "input": input, "cwd": "/work/proj"}).to_string()
    }

    /// Runs one input through both harness shapes; both must agree.
    fn verdict(tool: &str, input: Value) -> i32 {
        let (home, tack) = (Path::new(HOME), Path::new("/home/u/.tack"));
        let (c, msg) = judge(Harness::Claude, &claude(tool, &input), home, tack);
        let (p, _) = judge(Harness::Pi, &pi(tool, &input), home, tack);
        assert_eq!(c, p, "harness shapes disagree on {input}");
        if c == 2 {
            assert!(msg.starts_with("Blocked by tack guard: "), "{msg}");
        }
        c
    }

    fn cmd(c: &str) -> i32 {
        verdict("Bash", json!({ "command": c }))
    }

    fn file(field: &str, p: &str) -> i32 {
        verdict("Read", json!({ field: p }))
    }

    #[test]
    fn unparseable_input_is_denied() {
        let home = Path::new(HOME);
        let empty = json!({});
        assert_eq!(run(Harness::Claude, "not json", home).0, 2);
        assert_eq!(run(Harness::Pi, "", home).0, 2);
        assert_eq!(run(Harness::Pi, r#"{"tool":"read"}"#, home).0, 2);
        assert_eq!(
            run(Harness::Pi, r#"{"tool":"read","input":"x"}"#, home).0,
            2
        );
        assert_eq!(run(Harness::Claude, &pi("read", &empty), home).0, 2);
        assert_eq!(run(Harness::Pi, &claude("Read", &empty), home).0, 2);
        assert_eq!(
            run(Harness::Claude, &claude("Read", &empty), home),
            (0, String::new())
        );
    }

    #[test]
    fn credential_paths_are_denied() {
        assert_eq!(file("file_path", "~/.ssh/id_ed25519"), 2);
        assert_eq!(file("path", "/home/u/.ssh"), 2);
        assert_eq!(file("path", "$HOME/.aws/credentials"), 2);
        assert_eq!(file("file_path", "${HOME}/.gnupg/pubring.kbx"), 2);
        assert_eq!(file("path", "~/.config/gh/hosts.yml"), 2);
        assert_eq!(file("file_path", "~/.netrc"), 2);
        assert_eq!(file("file_path", "~/.npmrc"), 2);
        assert_eq!(file("file_path", "~/.docker/config.json"), 2);
        assert_eq!(file("file_path", "~/.kube/config"), 2);
        assert_eq!(file("file_path", "~/.claude/.credentials.json"), 2);
        assert_eq!(file("path", "~/.pi/agent/auth.json"), 2);
        assert_eq!(file("file_path", "~/.codex/auth.json"), 2);
        assert_eq!(file("notebook_path", "~/.aws/x.ipynb"), 2);
        assert_eq!(file("path", "../../home/u/.ssh/config"), 2);
        assert_eq!(file("path", "/home/u/proj/./../.ssh"), 2);
        assert_eq!(verdict("Glob", json!({"pattern": "~/.ssh/*"})), 2);
    }

    #[test]
    fn credential_near_misses_are_allowed() {
        assert_eq!(file("file_path", "README.md"), 0);
        assert_eq!(file("file_path", "~/.sshrc"), 0);
        assert_eq!(file("file_path", "~/.docker/other.json"), 0);
        assert_eq!(file("file_path", "/work/proj/.ssh/fixture"), 0);
        assert_eq!(file("path", "~/.config/ghostty"), 0);
        assert_eq!(file("path", "~"), 0);
        assert_eq!(verdict("Glob", json!({"pattern": "**/*.rs"})), 0);
    }

    #[test]
    fn env_files_are_denied() {
        assert_eq!(file("file_path", ".env"), 2);
        assert_eq!(file("file_path", "/work/proj/.env.local"), 2);
        assert_eq!(file("path", "app/.env.production"), 2);
        assert_eq!(verdict("Glob", json!({"pattern": "app/.env"})), 2);
        assert_eq!(cmd("cat .env"), 2);
        assert_eq!(cmd("cp .env.example .env"), 2);
    }

    #[test]
    fn env_near_misses_are_allowed() {
        assert_eq!(file("file_path", ".env.example"), 0);
        assert_eq!(file("file_path", "config/.env.sample"), 0);
        assert_eq!(file("file_path", ".env.template"), 0);
        assert_eq!(file("file_path", "src/environment.ts"), 0);
        assert_eq!(file("file_path", ".envrc"), 0);
        assert_eq!(cmd("cat .env.example"), 0);
        assert_eq!(
            verdict("Grep", json!({"pattern": ".env", "path": "src"})),
            0
        );
    }

    #[test]
    fn shell_naming_a_credential_path_is_denied() {
        assert_eq!(cmd("cat ~/.ssh/id_rsa"), 2);
        assert_eq!(cmd("ls -la $HOME/.aws"), 2);
        assert_eq!(cmd("echo hi && cat \"${HOME}/.netrc\""), 2);
        assert_eq!(cmd("grep token < ~/.npmrc"), 2);
        assert_eq!(cmd("cat foo | tee ~/.kube/config"), 2);
        assert_eq!(cmd("echo $(cat ~/.config/gh/hosts.yml)"), 2);
        assert_eq!(cmd("tool --key=~/.ssh/id_rsa"), 2);
        assert_eq!(cmd("bash -c 'cat ~/.ssh/id_rsa'"), 2);
        assert_eq!(cmd("eval cat ~/.aws/config"), 2);
    }

    #[test]
    fn shell_near_misses_are_allowed() {
        assert_eq!(cmd("cat README.md"), 0);
        assert_eq!(cmd("git commit -m \"docs: mention ~/.ssh handling\""), 0);
        assert_eq!(cmd("ls -la ~"), 0);
        assert_eq!(cmd("cargo test && cargo clippy"), 0);
    }

    #[test]
    fn keychain_reads_are_denied() {
        assert_eq!(cmd("security find-generic-password -s foo -w"), 2);
        assert_eq!(cmd("/usr/bin/security find-internet-password -s x"), 2);
        assert_eq!(cmd("echo x; security dump-keychain"), 2);
        assert_eq!(cmd("sudo security dump-keychain"), 2);
    }

    #[test]
    fn keychain_near_misses_are_allowed() {
        assert_eq!(cmd("security list-keychains"), 0);
        assert_eq!(cmd("echo find-generic-password"), 0);
    }

    #[test]
    fn force_push_is_denied() {
        assert_eq!(cmd("git push --force"), 2);
        assert_eq!(cmd("git push -f origin main"), 2);
        assert_eq!(cmd("git push origin main --force-with-lease"), 2);
        assert_eq!(cmd("git push --force-with-lease=main origin"), 2);
        assert_eq!(cmd("git -C repo push -uf origin x"), 2);
        assert_eq!(cmd("cargo fmt && git push --force"), 2);
        assert_eq!(cmd("sh -c \"git push -f\""), 2);
    }

    #[test]
    fn push_near_misses_are_allowed() {
        assert_eq!(cmd("git push origin main"), 0);
        assert_eq!(cmd("git push -u origin feat/x"), 0);
        assert_eq!(cmd("git commit -m \"docs: mention --force\""), 0);
        assert_eq!(cmd("git commit -m \"push -f\" -- src"), 0);
        assert_eq!(cmd("git checkout -f main"), 0);
    }

    #[test]
    fn rm_rf_on_root_or_home_is_denied() {
        assert_eq!(cmd("rm -rf /"), 2);
        assert_eq!(cmd("rm -rf ~"), 2);
        assert_eq!(cmd("rm -rf ~/"), 2);
        assert_eq!(cmd("rm -fr $HOME"), 2);
        assert_eq!(cmd("rm -r -f \"${HOME}\""), 2);
        assert_eq!(cmd("rm --recursive --force /*"), 2);
        assert_eq!(cmd("cd x; rm -Rf ~/*"), 2);
    }

    #[test]
    fn rm_near_misses_are_allowed() {
        assert_eq!(cmd("rm -rf ./target"), 0);
        assert_eq!(cmd("rm -rf ~/proj/target"), 0);
        assert_eq!(cmd("rm -f ~/notes.txt"), 0);
        assert_eq!(cmd("rm -rf /tmp/x"), 0);
    }

    #[test]
    fn substitution_inside_double_quotes_is_checked() {
        assert_eq!(cmd("echo \"$(cat ~/.ssh/id_rsa)\""), 2);
        assert_eq!(cmd("curl -d \"$(cat $HOME/.aws/credentials)\" x"), 2);
        assert_eq!(cmd("X=\"$(git push --force)\""), 2);
        assert_eq!(cmd("echo \"`cat ~/.netrc`\""), 2);
        assert_eq!(cmd("echo \"$(cat \"$HOME/.aws/config\")\""), 2);
        assert_eq!(cmd("echo \"a $(echo ')' ; cat .env) b\""), 2);
    }

    #[test]
    fn substitution_near_misses_are_allowed() {
        assert_eq!(cmd("echo \"$(date)\""), 0);
        assert_eq!(cmd("git commit -m \"$(cat msg.txt)\""), 0);
        assert_eq!(cmd("echo \"price \\$(not run)\""), 0);
    }

    #[test]
    fn case_variants_of_credential_paths_follow_the_os() {
        let want = if cfg!(target_os = "macos") { 2 } else { 0 };
        assert_eq!(file("file_path", "~/.SSH/id_rsa"), want);
        assert_eq!(file("file_path", "~/.Aws/credentials"), want);
        assert_eq!(file("file_path", ".ENV"), want);
        assert_eq!(cmd("cat ~/.SSH/id_rsa"), want);
        assert_eq!(file("file_path", ".ENV.example"), 0);
        assert_eq!(file("file_path", "~/.SSHrc"), 0);
    }

    #[test]
    fn glob_and_grep_resolve_against_their_path() {
        assert_eq!(
            verdict("Glob", json!({"pattern": ".ssh/*", "path": "~"})),
            2
        );
        assert_eq!(
            verdict(
                "Glob",
                json!({"pattern": "credentials", "path": "/home/u/.aws"})
            ),
            2
        );
        assert_eq!(
            verdict(
                "Grep",
                json!({"pattern": "k", "glob": ".aws/*", "path": "~"})
            ),
            2
        );
        assert_eq!(verdict("Grep", json!({"pattern": "k", "glob": ".env"})), 2);
    }

    #[test]
    fn glob_and_grep_near_misses_are_allowed() {
        assert_eq!(
            verdict("Glob", json!({"pattern": "src/*.rs", "path": "~"})),
            0
        );
        assert_eq!(
            verdict("Glob", json!({"pattern": ".ssh/*", "path": "/work"})),
            0
        );
        assert_eq!(
            verdict(
                "Grep",
                json!({"pattern": ".env", "glob": "*.rs", "path": "src"})
            ),
            0
        );
    }

    #[test]
    fn wrappers_and_git_global_options_are_skipped() {
        assert_eq!(cmd("sudo -E rm -rf /"), 2);
        assert_eq!(cmd("sudo -u root rm -rf /"), 2);
        assert_eq!(cmd("doas rm -rf /"), 2);
        assert_eq!(cmd("env -i git push -f"), 2);
        assert_eq!(cmd("timeout 30 git push -f"), 2);
        assert_eq!(cmd("timeout -s KILL 5s git push --force"), 2);
        assert_eq!(cmd("nice rm -rf ~"), 2);
        assert_eq!(cmd("nice -n 5 rm -rf ~"), 2);
        assert_eq!(cmd("xargs -0 git push -f"), 2);
        assert_eq!(cmd("git --git-dir .git push -f"), 2);
        assert_eq!(cmd("git --work-tree . --namespace x push -f"), 2);
    }

    // rtk's pi extension rewrites `git ...` to `rtk git ...` before the guard sees the call.
    #[test]
    fn rtk_prefix_is_a_wrapper() {
        assert_eq!(cmd("rtk git push --force origin main"), 2);
        assert_eq!(cmd("rtk proxy git push -f"), 2);
        assert_eq!(cmd("rtk rm -rf ~"), 2);
        assert_eq!(cmd("rtk git push origin main"), 0);
        assert_eq!(cmd("rtk git status"), 0);
        assert_eq!(cmd("rtk gain --history"), 0);
        assert_eq!(cmd("rtk proxy ls -la"), 0);
    }

    #[test]
    fn rtk_subcommands_cannot_hide_a_command() {
        for sub in [
            "bypass",
            "run",
            "err",
            "summary",
            "test",
            "proxy --skip-env",
            "proxy --",
        ] {
            assert_eq!(
                cmd(&format!("rtk {sub} git push --force origin main")),
                2,
                "{sub}"
            );
            assert_eq!(cmd(&format!("rtk {sub} rm -rf ~")), 2, "{sub}");
        }
        assert_eq!(cmd("rtk run -c 'git push --force origin main'"), 2);
        assert_eq!(cmd("rtk err --shell sh 'git push -f'"), 2);
        assert_eq!(cmd("rtk bypass security find-generic-password -s x"), 2);
        assert_eq!(cmd("rtk run sudo git push -f"), 2);
        assert_eq!(cmd("rtk run --command='git push --force origin main'"), 2);
        assert_eq!(cmd("rtk run --shell=bash --command='git push -f'"), 2);
        assert_eq!(cmd("rtk run --command=git\\ push\\ -f"), 2);
        assert_eq!(cmd("rtk run --command='rm -rf ~'"), 2);
        assert_eq!(cmd("rtk run 'cat<.env'"), 2);
        assert_eq!(cmd("rtk err --shell=sh 'cat<.env'"), 2);
        assert_eq!(cmd("rtk -v run 'cat<.env'"), 2);
        assert_eq!(cmd("rtk run -c'git push -f'"), 2);
        assert_eq!(cmd("rtk test --shell=sh -c'cat<.env'"), 2);
    }

    // pi rewrites most commands to `rtk <cmd>`, so text arguments must not read as commands.
    #[test]
    fn rtk_text_arguments_are_allowed() {
        assert_eq!(cmd("rtk grep \"git push --force\" src"), 0);
        assert_eq!(cmd("rtk rg 'git push -f' docs"), 0);
        assert_eq!(cmd("rtk grep -n \"rm -rf ~\" src"), 0);
        assert_eq!(cmd("rtk grep \".env loader\" src"), 0);
        assert_eq!(
            cmd("rtk gh issue comment 1 --body \"rm -rf ~ is denied\""),
            0
        );
        assert_eq!(cmd("rtk echo \"git push --force\""), 0);
        assert_eq!(cmd("rtk test cargo test -- --nocapture"), 0);
        assert_eq!(cmd("rtk run echo 'rm -rf ~ is blocked'"), 0);
        assert_eq!(cmd("rtk run echo \"git push --force is denied\""), 0);
        assert_eq!(cmd("rtk run git commit -m 'rm -rf ~ guard'"), 0);
        assert_eq!(cmd("rtk test npm test -- -t 'rm -rf ~'"), 0);
        assert_eq!(cmd("rtk test cargo test 'cat .env'"), 0);
        assert_eq!(cmd("rtk run grep -n '.env' src"), 0);
        assert_eq!(cmd("rtk run printf '%s\\n' 'cat .env'"), 0);
        assert_eq!(cmd("rtk test pytest -k 'test_env_loader or .env'"), 0);
    }

    #[test]
    fn program_names_follow_the_os_case_rule() {
        let want = if cfg!(target_os = "macos") { 2 } else { 0 };
        assert_eq!(cmd("GIT push -f"), want);
        assert_eq!(cmd("SUDO Git push --force"), want);
        assert_eq!(cmd("RTK run -c 'git push -f'"), want);
        assert_eq!(cmd("SH -c 'rm -rf ~'"), want);
        assert_eq!(cmd("Git status"), 0);
    }

    #[test]
    fn rtk_near_misses_are_allowed() {
        assert_eq!(cmd("rtk run cargo test"), 0);
        assert_eq!(cmd("rtk err npm run build"), 0);
        assert_eq!(cmd("rtk run -c 'cargo build && cargo test'"), 0);
        assert_eq!(cmd("rtk grep \"foo bar\" src"), 0);
        assert_eq!(
            cmd("rtk git commit -m \"docs: git push --force is blocked\""),
            0
        );
        assert_eq!(cmd("rtk bypass rm -rf ./build"), 0);
    }

    #[test]
    fn wrapper_near_misses_are_allowed() {
        assert_eq!(cmd("sudo -E rm -rf ./build"), 0);
        assert_eq!(cmd("timeout 30 git push"), 0);
        assert_eq!(cmd("env -i cargo test"), 0);
        assert_eq!(cmd("nice -n 5 cargo build"), 0);
        assert_eq!(cmd("git --git-dir .git push origin main"), 0);
        assert_eq!(cmd("git --work-tree . status -f"), 0);
    }

    #[test]
    fn env_file_names_as_text_are_allowed() {
        assert_eq!(cmd("echo \".env\" >> .gitignore"), 0);
        assert_eq!(cmd("printf '.env\\n' >> .gitignore"), 0);
        assert_eq!(cmd("cat >> .gitignore <<'EOF'\n.env\n.env.local\nEOF"), 0);
        assert_eq!(cmd("cat > notes.md <<'EOF'\n$(cat .env)\nEOF"), 0);
        assert_eq!(cmd("git rm --cached .env"), 0);
        assert_eq!(cmd("git check-ignore .env"), 0);
        assert_eq!(cmd("git ls-files .env .env.local"), 0);
        assert_eq!(cmd("grep -r \"\\.env\" src/"), 0);
        assert_eq!(cmd("grep -rn .env src/"), 0);
        assert_eq!(cmd("rg -e .env src"), 0);
        assert_eq!(cmd("cargo test 2>&1 | tail"), 0);
    }

    #[test]
    fn text_naming_a_credential_path_is_still_denied() {
        assert_eq!(cmd("echo ~/.ssh/id_rsa | xargs cat"), 2);
        assert_eq!(cmd("cat <<'EOF' | xargs cat\n~/.aws/credentials\nEOF"), 2);
        assert_eq!(cmd("printf '%s' $TACK_HOME/tack.toml | xargs rm"), 2);
    }

    #[test]
    fn env_file_reads_and_writes_are_still_denied() {
        assert_eq!(cmd("cat .env"), 2);
        assert_eq!(cmd("cp .env x"), 2);
        assert_eq!(cmd("> .env"), 2);
        assert_eq!(cmd("echo KEY=1 > .env"), 2);
        assert_eq!(cmd("printf x >> app/.env.local"), 2);
        assert_eq!(cmd("source .env"), 2);
        assert_eq!(cmd(". .env"), 2);
        assert_eq!(cmd("grep KEY .env"), 2);
        assert_eq!(cmd("grep -f .env src"), 2);
        assert_eq!(cmd("git rm .env"), 2);
        assert_eq!(cmd("git add .env"), 2);
        assert_eq!(cmd("cat <<EOF\n$(cat .env)\nEOF"), 2);
        assert_eq!(cmd("bash <<'EOF'\ngit push -f\nEOF"), 2);
        assert_eq!(cmd("cat <<'EOF' | sh\nrm -rf ~\nEOF"), 2);
        assert_eq!(cmd("bash <<< \"git push -f\""), 2);
    }

    #[test]
    fn heredoc_misparse_cannot_hide_a_command() {
        assert_eq!(cmd("echo $((1<<2))\ngit push -f"), 2);
        assert_eq!(cmd("true # <<EOF\ngit push --force"), 2);
        assert_eq!(cmd("x=1 # <<X\nrm -rf ~"), 2);
        assert_eq!(cmd("rm -rf ~ # cleanup"), 2);
        assert_eq!(cmd("((git push -f) )"), 2);
        assert_eq!(cmd("echo $(( $(cat ~/.ssh/id_rsa) ))"), 2);
        // Heredoc bodies are checked as commands too, even when only `cat` reads them.
        assert_eq!(
            cmd("cat > notes.md <<EOF\ngit push -f is bad\nEOF\necho ok"),
            2
        );
        assert_eq!(cmd("cat > n <<'EOF'\nrm -rf ~\nEOF"), 2);
    }

    #[test]
    fn comment_and_arithmetic_near_misses_are_allowed() {
        assert_eq!(cmd("echo $((1<<2))\necho ok"), 0);
        assert_eq!(cmd("cargo test # <<not a heredoc\necho ok"), 0);
        assert_eq!(cmd("echo a#b '#' \"#\" > x.txt"), 0);
        assert_eq!(cmd("echo \"$((1<<2))\""), 0);
        assert_eq!(cmd("cat > notes.md <<'EOF'\n# heading\n.env\nEOF"), 0);
    }

    #[test]
    fn echo_feeding_xargs_or_a_shell_is_a_path() {
        assert_eq!(cmd("echo .env | xargs cat"), 2);
        assert_eq!(cmd("printf '.env.local' | sudo xargs -0 rm"), 2);
        assert_eq!(cmd("echo .env | sort | sh"), 2);
        assert_eq!(cmd("(echo .env) |& xargs cat"), 2);
        assert_eq!(cmd("env -S'git push -f'"), 2);
        assert_eq!(cmd("echo .env | tee -a .gitignore"), 0);
        assert_eq!(cmd("echo .env >> .gitignore; ls | xargs wc -l"), 0);
        assert_eq!(cmd("echo .env >> .gitignore || xargs true"), 0);
    }

    #[test]
    fn long_wrapper_flags_are_skipped() {
        assert_eq!(cmd("sudo --user root git push -f"), 2);
        assert_eq!(cmd("sudo --chdir /tmp rm -rf /"), 2);
        assert_eq!(cmd("env -S \"git push -f\""), 2);
        assert_eq!(cmd("env --split-string='git push --force'"), 2);
        assert_eq!(cmd("env --split-string 'rm -rf ~'"), 2);
        assert_eq!(cmd("sudo --user root cargo build"), 0);
        assert_eq!(cmd("env -S \"cargo test\""), 0);
    }

    #[test]
    fn force_push_by_refspec_is_denied() {
        assert_eq!(cmd("git push origin +main"), 2);
        assert_eq!(cmd("git push origin +HEAD:main"), 2);
        assert_eq!(cmd("git push origin main:main"), 0);
        assert_eq!(cmd("git commit -m x +main"), 0);
    }

    #[test]
    fn nesting_is_capped() {
        assert_eq!(cmd(&format!("{}echo hi", "eval ".repeat(9))), 2);
        assert_eq!(cmd(&format!("{}git push -f", "eval ".repeat(8))), 2);
        assert_eq!(cmd(&format!("{}echo hi", "eval ".repeat(8))), 0);
        assert_eq!(cmd(&format!("{}git push -f", "eval ".repeat(10_000))), 2);
        assert_eq!(cmd("eval eval echo hi"), 0);
        let quoted = |n| (0..n).fold("echo hi".to_string(), |c, _| format!("echo \"$({c})\""));
        assert_eq!(cmd(&quoted(9)), 2);
        assert_eq!(cmd(&quoted(8)), 0);
        assert_eq!(cmd(&quoted(3).replace("echo hi", "cat .env")), 2);
    }

    fn write(p: &str) -> i32 {
        verdict("Write", json!({ "file_path": p, "content": "x" }))
    }

    #[test]
    fn tack_files_are_protected() {
        assert_eq!(write("~/.tack/state.toml"), 2);
        assert_eq!(write("/home/u/.tack/bin/tack"), 2);
        assert_eq!(write("$TACK_HOME/tack.toml"), 2);
        assert_eq!(
            verdict(
                "Edit",
                json!({"file_path": "~/.pi/agent/extensions/tack-guard.ts"})
            ),
            2
        );
        assert_eq!(cmd("cat ~/.tack/state.toml"), 2);
        assert_eq!(cmd("rm -rf ~/.tack"), 2);
        assert_eq!(cmd("echo x > ~/.tack/tack.toml"), 2);
        assert_eq!(cmd("sed -i s/a/b/ \"$TACK_HOME/tack.toml\""), 2);
        assert_eq!(cmd("rm ~/.pi/agent/extensions/tack-guard.ts"), 2);
        assert_eq!(cmd("~/.tack/store/x/tack doctor"), 2);
    }

    #[test]
    fn tack_near_misses_are_allowed() {
        assert_eq!(file("file_path", "~/.tack/state.toml"), 0);
        assert_eq!(
            verdict("Glob", json!({"pattern": "*", "path": "~/.tack"})),
            0
        );
        assert_eq!(write("~/.tackle/x"), 0);
        assert_eq!(write("~/.pi/agent/extensions/other.ts"), 0);
        // Stated limit: the owner edits Claude settings through the agent.
        assert_eq!(write("~/.claude/settings.json"), 0);
        assert_eq!(cmd("~/.tack/bin/tack doctor"), 0);
        assert_eq!(cmd("tack install && tack list"), 0);
        assert_eq!(cmd("git commit -m \"docs: mention ~/.tack\""), 0);
    }

    #[test]
    fn tack_home_comes_from_the_caller() {
        let stdin = claude("Bash", &json!({"command": "cat /opt/t/state.toml"}));
        let home = Path::new(HOME);
        assert_eq!(
            judge(Harness::Claude, &stdin, home, Path::new("/opt/t")).0,
            2
        );
        assert_eq!(
            judge(Harness::Claude, &stdin, home, Path::new("/opt/u")).0,
            0
        );
    }

    #[test]
    fn splitting_respects_quotes_and_operators() {
        let words = |ws: &[&str]| ws.iter().map(|w| w.to_string()).collect::<Vec<_>>();
        let seg = |ws: &[&str], rs: &[&str]| Seg {
            words: words(ws),
            redirects: words(rs),
            piped: false,
        };
        let p = split_command(
            "a 'b c' \"d\\\"e\\.f\"&&f|g;h\ni>j '' 2>&1 <<-X\n\tbody $(cmd)\n\tX\nk||l # m\nn",
        );
        let mut want = vec![
            seg(&["a", "b c", "d\"e\\.f"], &[]),
            seg(&["f"], &[]),
            seg(&["g"], &[]),
            seg(&["h"], &[]),
            seg(&["i", "", "2"], &["j", "1"]),
            seg(&["k"], &[]),
            seg(&["l"], &[]),
            seg(&["n"], &[]),
        ];
        want[1].piped = true;
        assert_eq!(p.segs, want);
        assert_eq!(p.stdin, ["\tbody $(cmd)\n"]);
        assert_eq!(p.subs, ["cmd"]);
    }
}
