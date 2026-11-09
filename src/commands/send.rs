//! `meow send` — put your saves online, and set up a home if there isn't one.

use std::process::{Command, Stdio};

use anyhow::{anyhow, Result};
use serde_json::json;

use crate::git::{classify, GitError};
use crate::journal::{Action, Journal};
use crate::repo::{pretty_remote, Repo};
use crate::ui::{Sym, Ui};
use crate::util::require_repo;

pub fn run(ui: &Ui) -> Result<i32> {
    let repo = require_repo(ui)?;
    let status = repo.status()?;
    if status.conflicted() {
        ui.failure("A combine still needs your help.");
        ui.next(&[("meow resolve", "finish it first")]);
        return Ok(1);
    }
    let Some(branch) = status.branch.clone() else {
        ui.failure("You're looking at an old save, not a branch.");
        ui.next(&[("meow branch main", "go back to your branch")]);
        return Ok(1);
    };
    if !repo.has_commits() {
        ui.failure("There's nothing to send yet.");
        ui.next(&[("meow save \"First save\"", "save your work first")]);
        return Ok(1);
    }
    let Some(home) = repo.home_remote() else {
        return publish(ui, &repo, &branch);
    };

    push(
        ui,
        &repo,
        &home.name,
        &branch,
        status.upstream.is_some(),
        status.ahead,
    )
}

fn push(
    ui: &Ui,
    repo: &Repo,
    remote: &str,
    branch: &str,
    has_upstream: bool,
    ahead: usize,
) -> Result<i32> {
    let label = repo
        .remotes()
        .into_iter()
        .find(|r| r.name == remote)
        .map(|r| pretty_remote(&r.url))
        .unwrap_or_else(|| remote.to_string());

    let count = if has_upstream {
        ahead
    } else {
        repo.git
            .probe(&[
                "rev-list",
                "--count",
                &format!("refs/remotes/{remote}/{branch}..HEAD"),
            ])
            .and_then(|s| s.parse().ok())
            .unwrap_or_else(|| repo.commit_count())
    };
    if count == 0 && has_upstream {
        if ui.plain {
            println!("sent {branch} {label} 0 saves");
        } else {
            ui.line(format!(
                "{} Everything is already online.",
                ui.sym(Sym::Success)
            ));
        }
        return Ok(0);
    }

    if !ui.plain {
        ui.line(format!(
            "Sending {count} {} to {label}…",
            if count == 1 { "save" } else { "saves" }
        ));
        ui.blank();
    }

    let args = if has_upstream {
        vec!["push"]
    } else {
        vec!["push", "-u", remote, branch]
    };
    let out = repo.git.run_main(&args)?;
    if !out.success() {
        return handle_push_failure(ui, repo, &out, remote, branch, count);
    }

    if let Some(head) = repo.head() {
        let _ = repo.git.run(&[
            "update-ref",
            &format!("refs/meow/last-sent/{branch}"),
            &head,
        ]);
    }
    let mut journal = Journal::load(&repo.git_dir);
    let mut action = Action::new(
        "send",
        format!(
            "Sent {count} {} to {label}",
            if count == 1 { "save" } else { "saves" }
        ),
    );
    action.head_before = repo.head();
    action.head_after = repo.head();
    action.data = json!({ "remote": remote, "branch": branch, "count": count, "label": label });
    let _ = journal.record(&repo.git_dir, action);

    if ui.plain {
        println!("sent {branch} {label} {count} saves");
        return Ok(0);
    }
    ui.success("Sent. Your team can see your work now.");
    ui.blank();
    ui.line(ui.dim(format!("{branch} is up to date.")));
    let status = repo.status()?;
    let incoming = crate::commands::home::incoming_count(repo, &status);
    if incoming > 0 {
        ui.next(&[("meow get", "get their new work")]);
    }
    Ok(0)
}

fn handle_push_failure(
    ui: &Ui,
    repo: &Repo,
    out: &crate::git::GitOutput,
    remote: &str,
    branch: &str,
    count: usize,
) -> Result<i32> {
    let err = GitError {
        stdout: out.stdout.clone(),
        stderr: out.stderr.clone(),
    };
    let failure = classify(&err);
    match failure {
        crate::git::Failure::Rejected => {
            ui.failure("Someone shared new work after your last get.");
            ui.blank();
            ui.line("Your saves are safe — nothing was lost.");
            if ui.can_prompt() {
                let go = ui.confirm("Get their work, then send yours?", true)?;
                if go {
                    let code = crate::commands::get::run(ui, None, None)?;
                    if code != 0 {
                        return Ok(code);
                    }
                    return push(ui, repo, remote, branch, true, count);
                }
            } else {
                return Err(ui.decision_required(
                    "Someone shared new work after your last get.",
                    "Run `meow get`, then `meow send`.",
                ));
            }
            ui.next(&[
                ("meow get", "get their work"),
                ("meow send", "then send yours"),
            ]);
            Ok(1)
        }
        crate::git::Failure::Protected => protected(ui, repo, remote, branch, count),
        crate::git::Failure::Authentication => {
            ui.failure("Your login for this host didn't work.");
            ui.blank();
            ui.line(crate::git::Failure::Authentication.hint());
            Ok(1)
        }
        crate::git::Failure::Offline => {
            ui.failure("Meow couldn't reach the online home.");
            ui.blank();
            ui.line("Your saves are safe here. Try again when you're online.");
            Ok(1)
        }
        crate::git::Failure::NotFound => {
            ui.failure("That project couldn't be found online.");
            ui.blank();
            ui.line("Check that the home still exists, or run `meow doctor`.");
            Ok(1)
        }
        _ => {
            ui.failure(failure.message());
            if !out.stderr.trim().is_empty() {
                ui.blank();
                for line in out.stderr.lines().take(16) {
                    ui.line(ui.dim(line));
                }
            }
            Ok(1)
        }
    }
}

fn protected(ui: &Ui, repo: &Repo, remote: &str, branch: &str, _count: usize) -> Result<i32> {
    ui.failure(format!("{branch} is protected on this host."));
    ui.blank();
    ui.line("Meow can put your saves on a new branch and send that instead.");
    let new_branch = format!(
        "meow/{}-{}",
        branch.replace('/', "-"),
        chrono::Local::now().format("%Y%m%d-%H%M")
    );
    if !ui.can_prompt() {
        return Err(ui.decision_required(
            "That branch is protected on this host.",
            &format!("Create and send a new branch: meow branch {new_branch} && meow send"),
        ));
    }
    if !ui.confirm(&format!("Create and send `{new_branch}`?"), true)? {
        ui.line("Nothing was sent.");
        return Ok(1);
    }
    repo.git.check(&["switch", "-c", &new_branch])?;
    let out = repo.git.run_main(&["push", "-u", remote, &new_branch])?;
    if !out.success() {
        ui.failure("The new branch couldn't be sent.");
        for line in out.stderr.lines().take(12) {
            ui.line(ui.dim(line));
        }
        return Ok(1);
    }
    ui.success(format!("Sent your saves on `{new_branch}`."));
    ui.blank();
    ui.line("Open a pull request on the host to ask for them to be combined.");
    Ok(0)
}

// ---------------------------------------------------------------- publishing

fn publish(ui: &Ui, repo: &Repo, branch: &str) -> Result<i32> {
    if ui.plain {
        return Err(ui.decision_required(
            "This project doesn't have an online home yet.",
            "Create one, then add it: git remote add origin <link> && meow send",
        ));
    }
    ui.line("This project isn't online yet.");
    ui.blank();
    ui.line("An online copy lets you back up your work and share it.");
    ui.blank();
    let choice = ui.select(
        "Where should it live?",
        &[
            "GitHub (recommended, uses your gh login)".to_string(),
            "I already made one — paste the link".to_string(),
            "Not now".to_string(),
        ],
        0,
    )?;
    match choice {
        0 => publish_github(ui, repo, branch),
        1 => link_existing(ui, repo, branch),
        _ => {
            ui.line("Nothing was sent. Your saves are safe here.");
            Ok(0)
        }
    }
}

fn publish_github(ui: &Ui, repo: &Repo, branch: &str) -> Result<i32> {
    if !gh_available() {
        ui.warning("The GitHub CLI (gh) isn't installed.");
        ui.line("You can still paste a link to a repository you made on github.com.");
        return link_existing(ui, repo, branch);
    }
    if !gh_authenticated() {
        ui.warning("The GitHub CLI isn't signed in.");
        if ui.confirm("Sign in now?", true)? {
            let status = Command::new("gh")
                .args(["auth", "login", "--web", "--git-protocol", "https"])
                .stdin(Stdio::inherit())
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .status();
            if !matches!(status, Ok(s) if s.success()) {
                ui.failure("Signing in didn't finish.");
                return Ok(1);
            }
        } else {
            return link_existing(ui, repo, branch);
        }
    }

    let default_name = repo
        .root
        .file_name()
        .map(|s| crate::util::slugify(&s.to_string_lossy()))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "my-project".to_string());
    let name = ui.input("Repository name", &default_name)?;
    let name = if name.trim().is_empty() {
        default_name
    } else {
        name.trim().to_string()
    };
    let description = ui.input("Description (optional)", "")?;
    let visibility = ui.select(
        "Visibility",
        &[
            "Public — anyone can see it".to_string(),
            "Private — only you and people you invite".to_string(),
        ],
        0,
    )?;
    let owner = gh_login().unwrap_or_else(|| "you".to_string());
    let label = format!("github.com/{owner}/{name}");
    if !ui.confirm(&format!("Create {label}?"), true)? {
        ui.line("Nothing was created. Your saves are safe here.");
        return Ok(0);
    }

    ui.blank();
    ui.line(format!("Creating {label}…"));
    let mut cmd = Command::new("gh");
    cmd.args([
        "repo",
        "create",
        &name,
        "--source",
        ".",
        "--remote",
        "origin",
        "--push",
        if visibility == 0 {
            "--public"
        } else {
            "--private"
        },
    ]);
    if !description.trim().is_empty() {
        cmd.args(["--description", description.trim()]);
    }
    let out = cmd
        .current_dir(&repo.root)
        .output()
        .map_err(|e| anyhow!("Couldn't run gh: {e}"))?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        ui.failure("GitHub couldn't create the repository.");
        for line in stderr.lines().take(10) {
            ui.line(ui.dim(line));
        }
        ui.blank();
        ui.line("You can make it on github.com and paste the link instead.");
        if ui.confirm("Paste a link now?", false)? {
            return link_existing(ui, repo, branch);
        }
        return Ok(1);
    }

    let count = repo.commit_count();
    let mut journal = Journal::load(&repo.git_dir);
    let mut action = Action::new("send", format!("Created {label} and sent {count} saves"));
    action.head_before = repo.head();
    action.head_after = repo.head();
    action.data = json!({ "label": label, "count": count, "created": true });
    let _ = journal.record(&repo.git_dir, action);

    ui.success(format!("Created {label}"));
    ui.success(format!(
        "Sent {count} {}",
        if count == 1 { "save" } else { "saves" }
    ));
    ui.blank();
    ui.line(ui.dim(format!("Anyone with the link can see {branch}.")));
    Ok(0)
}

fn link_existing(ui: &Ui, repo: &Repo, branch: &str) -> Result<i32> {
    let link = ui.input("Project link", "")?;
    if link.trim().is_empty() {
        ui.line("Nothing was added.");
        return Ok(0);
    }
    let url = crate::repo::normalize_link(&link);
    if !ui.confirm(
        &format!("Use {} as this project's home?", pretty_remote(&url)),
        true,
    )? {
        ui.line("Nothing was added.");
        return Ok(0);
    }
    let has_origin = repo.remotes().iter().any(|r| r.name == "origin");
    if has_origin {
        repo.git.check(&["remote", "set-url", "origin", &url])?;
    } else {
        repo.git.check(&["remote", "add", "origin", &url])?;
    }
    let out = repo.git.run_main(&["push", "-u", "origin", branch])?;
    if !out.success() {
        let err = GitError {
            stdout: out.stdout.clone(),
            stderr: out.stderr.clone(),
        };
        let failure = classify(&err);
        ui.failure(failure.message());
        ui.line(failure.hint());
        return Ok(1);
    }
    let count = repo.commit_count();
    let mut journal = Journal::load(&repo.git_dir);
    let mut action = Action::new(
        "send",
        format!("Linked {} and sent {count} saves", pretty_remote(&url)),
    );
    action.data = json!({ "label": pretty_remote(&url), "count": count });
    let _ = journal.record(&repo.git_dir, action);
    ui.success(format!("Linked {}", pretty_remote(&url)));
    ui.success(format!(
        "Sent {count} {}",
        if count == 1 { "save" } else { "saves" }
    ));
    Ok(0)
}

fn gh_available() -> bool {
    Command::new("gh")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn gh_authenticated() -> bool {
    Command::new("gh")
        .args(["auth", "status"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn gh_login() -> Option<String> {
    let out = Command::new("gh")
        .args(["api", "user", "--jq", ".login"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let login = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if login.is_empty() {
        None
    } else {
        Some(login)
    }
}
