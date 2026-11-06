//! The Git runner.
//!
//! Meow never re-implements Git. It drives the user's installed `git` binary so
//! that hooks, signing, credentials, attributes, and configuration behave
//! exactly as they would with plain Git.

use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{anyhow, Result};

use crate::ui::Ui;

#[derive(Debug)]
pub struct GitError {
    pub command: String,
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

impl fmt::Display for GitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let first = self
            .stderr
            .lines()
            .map(str::trim)
            .find(|l| !l.is_empty())
            .unwrap_or("git failed");
        write!(f, "{}", clean_git_message(first))
    }
}

impl std::error::Error for GitError {}

fn clean_git_message(line: &str) -> String {
    let line = line
        .strip_prefix("fatal: ")
        .or_else(|| line.strip_prefix("error: "))
        .unwrap_or(line)
        .trim();
    let mut chars = line.chars();
    match chars.next() {
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

#[derive(Debug, Clone)]
pub struct GitOutput {
    pub stdout: String,
    pub stderr: String,
    pub code: Option<i32>,
    pub command: String,
}

impl GitOutput {
    pub fn success(&self) -> bool {
        self.code == Some(0)
    }
}

pub struct Git {
    pub cwd: PathBuf,
    ui: Ui,
}

impl Git {
    pub fn new(cwd: impl Into<PathBuf>, ui: &Ui) -> Self {
        Git {
            cwd: cwd.into(),
            ui: Ui::new(ui.plain, !ui.color, ui.ascii, ui.explain, ui.verbose),
        }
    }

    /// Run git and capture everything. Explains the call when `--explain` is on.
    pub fn run(&self, args: &[&str]) -> Result<GitOutput> {
        let command = render(args);
        let out = Command::new("git")
            .args(args)
            .current_dir(&self.cwd)
            .env("GIT_PAGER", "cat")
            .env("GIT_OPTIONAL_LOCKS", "0")
            .output()
            .map_err(|e| match e.kind() {
                std::io::ErrorKind::NotFound => anyhow!(
                    "Git isn't installed, or it isn't on your PATH. Install Git, then try again."
                ),
                _ => anyhow!("Couldn't run Git: {e}"),
            })?;
        let stdout = String::from_utf8_lossy(&out.stdout).to_string();
        let stderr = String::from_utf8_lossy(&out.stderr).to_string();
        Ok(GitOutput {
            stdout,
            stderr,
            code: out.status.code(),
            command,
        })
    }

    /// Run a headline operation and show it under the result with `--explain`.
    pub fn run_main(&self, args: &[&str]) -> Result<GitOutput> {
        let out = self.run(args)?;
        self.ui.explain(&out.command);
        Ok(out)
    }

    /// Run git with extra environment variables (used for private temp indexes).
    pub fn run_env(&self, args: &[&str], envs: &[(&str, &std::ffi::OsStr)]) -> Result<GitOutput> {
        let command = render(args);
        let mut cmd = Command::new("git");
        cmd.args(args)
            .current_dir(&self.cwd)
            .env("GIT_PAGER", "cat")
            .env("GIT_OPTIONAL_LOCKS", "0");
        for (k, v) in envs {
            cmd.env(k, v);
        }
        let out = cmd.output().map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => anyhow!(
                "Git isn't installed, or it isn't on your PATH. Install Git, then try again."
            ),
            _ => anyhow!("Couldn't run Git: {e}"),
        })?;
        Ok(GitOutput {
            stdout: String::from_utf8_lossy(&out.stdout).to_string(),
            stderr: String::from_utf8_lossy(&out.stderr).to_string(),
            code: out.status.code(),
            command,
        })
    }

    /// Run git, returning output only on success.
    pub fn check(&self, args: &[&str]) -> Result<GitOutput> {
        let out = self.run(args)?;
        if out.success() {
            Ok(out)
        } else {
            Err(anyhow::Error::new(GitError {
                command: out.command,
                code: out.code,
                stdout: out.stdout,
                stderr: out.stderr,
            }))
        }
    }

    /// Run git, returning trimmed stdout on success.
    pub fn out(&self, args: &[&str]) -> Result<String> {
        let out = self.check(args)?;
        Ok(out.stdout.trim_end_matches(['\n', '\r']).to_string())
    }

    /// Run git, returning trimmed stdout, or `None` if it failed. Used for probes.
    pub fn probe(&self, args: &[&str]) -> Option<String> {
        self.run(args).ok().filter(|o| o.success()).map(|o| {
            o.stdout
                .trim_end_matches(['\n', '\r'])
                .to_string()
        })
    }

    pub fn probe_bool(&self, args: &[&str]) -> bool {
        self.run(args).map(|o| o.success()).unwrap_or(false)
    }

    /// Run git with the current directory set elsewhere.
    pub fn at(&self, dir: impl Into<PathBuf>) -> Git {
        Git {
            cwd: dir.into(),
            ui: self.clone_ui(),
        }
    }

    fn clone_ui(&self) -> Ui {
        Ui::new(
            self.ui.plain,
            !self.ui.color,
            self.ui.ascii,
            self.ui.explain,
            self.ui.verbose,
        )
    }

    pub fn ui(&self) -> &Ui {
        &self.ui
    }

    pub fn root(&self) -> &Path {
        &self.cwd
    }

    pub fn git_dir(&self) -> Option<PathBuf> {
        self.probe(&["rev-parse", "--absolute-git-dir"])
            .map(PathBuf::from)
    }

    pub fn is_repo(&self) -> bool {
        self.probe(&["rev-parse", "--git-dir"]).is_some()
    }

    pub fn head(&self) -> Option<String> {
        self.probe(&["rev-parse", "HEAD"])
    }

    pub fn has_commits(&self) -> bool {
        self.head().is_some()
    }

    pub fn branch_name(&self) -> Option<String> {
        self.probe(&["symbolic-ref", "--quiet", "--short", "HEAD"])
    }
}

pub fn render(args: &[&str]) -> String {
    let mut out = String::from("git");
    for a in args {
        out.push(' ');
        if a.is_empty() || a.chars().any(char::is_whitespace) {
            out.push('"');
            out.push_str(&a.replace('"', "\\\""));
            out.push('"');
        } else {
            out.push_str(a);
        }
    }
    out
}

/// A short, human classification of a Git failure, used by friendly errors.
pub fn classify(err: &GitError) -> Failure {
    let s = format!("{}\n{}", err.stdout, err.stderr).to_lowercase();
    if s.contains("could not resolve host")
        || s.contains("connection timed out")
        || s.contains("network is unreachable")
        || s.contains("unable to access")
        || s.contains("could not read from remote repository")
    {
        return Failure::Offline;
    }
    if s.contains("authentication failed")
        || s.contains("invalid username or password")
        || s.contains("permission denied (publickey)")
        || s.contains("terminal prompts disabled")
        || s.contains("403")
    {
        return Failure::Authentication;
    }
    if s.contains("non-fast-forward")
        || s.contains("fetch first")
        || s.contains("rejected")
        || s.contains("failed to push some refs")
    {
        return Failure::Rejected;
    }
    if s.contains("protected branch") || s.contains("branch is protected") {
        return Failure::Protected;
    }
    if s.contains("does not appear to be a git repository") || s.contains("repository not found") {
        return Failure::NotFound;
    }
    if s.contains("no such file or directory") && s.contains(".git") {
        return Failure::NotARepo;
    }
    if s.contains("not possible to fast-forward") || s.contains("diverging branches") {
        return Failure::Diverged;
    }
    if s.contains("please tell me who you are")
        || s.contains("unable to auto-detect email address")
        || s.contains("empty ident name")
    {
        return Failure::Identity;
    }
    if s.contains("pre-commit") || s.contains("hook") && s.contains("failed") {
        return Failure::Hook;
    }
    if s.contains("no space left on device") {
        return Failure::DiskFull;
    }
    if s.contains("index.lock") || s.contains("unable to create") && s.contains("lock") {
        return Failure::Locked;
    }
    Failure::Other
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Failure {
    Offline,
    Authentication,
    Rejected,
    Protected,
    NotFound,
    NotARepo,
    Diverged,
    Identity,
    Hook,
    DiskFull,
    Locked,
    Other,
}

impl Failure {
    /// A plain-language sentence for the first line of an error screen.
    pub fn message(self) -> &'static str {
        match self {
            Failure::Offline => "Meow couldn't reach the online home.",
            Failure::Authentication => "Your login for this host didn't work.",
            Failure::Rejected => "Someone shared new work after your last get.",
            Failure::Protected => "That branch is protected on this host.",
            Failure::NotFound => "That project couldn't be found.",
            Failure::NotARepo => "This folder isn't a project yet.",
            Failure::Diverged => "Your work and the online copy have both changed.",
            Failure::Identity => "Meow needs a name to put on your saves.",
            Failure::Hook => "A project check blocked this action.",
            Failure::DiskFull => "There isn't enough space to finish.",
            Failure::Locked => "Another Git process is using this project.",
            Failure::Other => "That didn't work.",
        }
    }

    pub fn hint(self) -> &'static str {
        match self {
            Failure::Offline => "Check your connection, then try again.",
            Failure::Authentication => "Run `meow doctor --fix` to check your login.",
            Failure::Rejected => "Run `meow get`, then `meow send`.",
            Failure::Protected => "Run `meow doctor` for how to send on a new branch.",
            Failure::NotFound => "Check the link, or ask the owner for access.",
            Failure::NotARepo => "Run `meow new` or `meow get <link>`.",
            Failure::Diverged => "Run `meow get` to combine them safely.",
            Failure::Identity => "Run `meow doctor --fix` to set your name and email.",
            Failure::Hook => "Read the check output above, fix it, then try again.",
            Failure::DiskFull => "Free some space, then try again.",
            Failure::Locked => "Wait a moment, then try again.",
            Failure::Other => "Run `meow doctor` if this keeps happening.",
        }
    }
}
