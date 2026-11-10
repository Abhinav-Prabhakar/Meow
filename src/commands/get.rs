//! `meow get` — bring in new saves, or copy a project to this machine.

use std::fs;
use std::path::Path;

use anyhow::Result;
use serde_json::json;

use crate::git::{classify, GitError};
use crate::journal::{self, Action, Journal};
use crate::repo::{folder_name_from_link, normalize_link, pretty_remote, Repo};
use crate::ui::{Sym, Ui};
use crate::util::{author_label, relative_time, require_repo};

pub fn run(ui: &Ui, link: Option<&str>, folder: Option<&str>) -> Result<i32> {
    match link {
        Some(link) => clone_project(ui, link, folder),
        None => pull_project(ui),
    }
}

fn clone_project(ui: &Ui, link: &str, folder: Option<&str>) -> Result<i32> {
    let url = normalize_link(link);
    let name = folder
        .map(str::to_string)
        .unwrap_or_else(|| folder_name_from_link(&url));
    let dest = std::env::current_dir()?.join(&name);
    if dest.exists() && !is_empty(&dest) {
        ui.failure(format!("{} already exists and isn't empty.", name));
        ui.line("Pick another folder with `meow get <link> --folder <name>`.");
        return Ok(1);
    }
    let label = pretty_remote(&url);
    if ui.plain {
        println!("getting {label} into {name}");
    } else {
        ui.line(format!("Getting {label}…"));
        ui.blank();
    }

    let cwd = std::env::current_dir()?;
    let git = crate::git::Git::new(&cwd, ui);
    let out = git.run_progress(&["clone", "--progress", &url, &name], "Getting")?;
    if !out.success() {
        let err = GitError {
            stdout: out.stdout.clone(),
            stderr: out.stderr.clone(),
        };
        let failure = classify(&err);
        ui.failure(failure.message());
        if failure == crate::git::Failure::Other {
            for line in out.stderr.lines().take(10) {
                ui.line(ui.dim(line));
            }
        }
        ui.blank();
        ui.line(failure.hint());
        return Ok(1);
    }

    std::env::set_current_dir(&dest)?;
    let repo = Repo::discover(&dest, ui)?;
    let count = repo.as_ref().map(|r| r.commit_count()).unwrap_or(0);
    if ui.plain {
        println!("got {name} {}", crate::util::count(count, "save"));
        return Ok(0);
    }

    let info = repo.as_ref().and_then(|r| r.head_info().ok());
    ui.success(format!(
        "Got {} {}.",
        count,
        if count == 1 { "save" } else { "saves" }
    ));
    if let Some(info) = info {
        ui.line(format!(
            "  Last one: \"{}\" — {} · {}",
            info.subject,
            author_label(&info.author, None),
            relative_time(info.when)
        ));
    }
    ui.blank();
    crate::commands::home::run(ui)?;
    Ok(0)
}

fn pull_project(ui: &Ui) -> Result<i32> {
    let repo = require_repo(ui)?;
    let status = repo.status()?;
    let Some(branch) = status.branch.clone() else {
        ui.failure("You're looking at an old save, not a branch.");
        let back = format!("meow branch {}", repo.suggested_branch());
        ui.next(&[(back.as_str(), "go back to your branch")]);
        return Ok(1);
    };
    let Some(home) = repo.home_remote() else {
        ui.failure("This project doesn't have an online home yet.");
        ui.next(&[("meow send", "put your project online")]);
        return Ok(1);
    };
    let label = pretty_remote(&home.url);

    if status.conflicted() {
        ui.failure("A combine still needs your help before getting more.");
        ui.next(&[("meow resolve", "finish it first")]);
        return Ok(1);
    }

    let snapshot = journal::create_snapshot(&repo, "Before getting new saves")?;
    let head_before = repo.head();

    // Set aside unsaved changes, announced in one line.
    let mut stashed = false;
    if status.dirty() {
        if !ui.plain {
            ui.line(format!(
                "Setting aside {} unsaved {} while I get the latest…",
                status.files.len(),
                if status.files.len() == 1 {
                    "change"
                } else {
                    "changes"
                }
            ));
        }
        let out = repo.git.run(&[
            "stash",
            "push",
            "--include-untracked",
            "-m",
            "meow: set aside before get",
        ])?;
        if !out.success() {
            ui.failure("Meow couldn't set your unsaved changes aside.");
            ui.line(ui.dim(out.stderr.trim()));
            return Ok(1);
        }
        stashed = true;
    }

    if !ui.plain {
        ui.line(format!("Getting the latest from {label}…"));
        ui.blank();
    }

    let fetch = repo
        .git
        .run_progress(&["fetch", "--progress", &home.name], "Getting")?;
    if !fetch.success() {
        if stashed {
            let _ = repo.git.run(&["stash", "pop", "--quiet"]);
        }
        return report_git(ui, &fetch, "Meow couldn't reach the online home.");
    }

    let upstream = repo
        .git
        .probe(&["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"]);
    let target = upstream
        .clone()
        .unwrap_or_else(|| format!("refs/remotes/{}/{}", home.name, branch));
    if repo
        .git
        .probe(&["rev-parse", "--verify", &target])
        .is_none()
    {
        if stashed {
            let _ = repo.git.run(&["stash", "pop", "--quiet"]);
        }
        ui.line("The home doesn't have any saves yet.");
        ui.next(&[("meow send", "put your saves online")]);
        return Ok(0);
    }

    let merge = repo.git.run_main(&["merge", "--no-edit", &target])?;
    let head_after = repo.head();
    if !merge.success() {
        let err = GitError {
            stdout: merge.stdout.clone(),
            stderr: merge.stderr.clone(),
        };
        let failure = classify(&err);
        if repo.status()?.conflicted() {
            if !ui.plain {
                ui.failure("Some files need your help.");
                ui.blank();
                ui.line("You and someone else changed the same parts. Nothing is lost.");
                if stashed {
                    ui.line("Your unsaved changes are set aside safely and come back after the combine.");
                }
                ui.next(&[
                    ("meow resolve", "fix them step by step"),
                    ("meow cancel", "stop and go back"),
                ]);
            } else {
                println!("conflict");
            }
            record_conflict(&repo, &home.name, &branch, &label, head_before, snapshot);
            return Ok(1);
        }
        if stashed {
            let _ = repo.git.run(&["stash", "pop", "--quiet"]);
        }
        return report_git(ui, &merge, failure.message());
    }

    if upstream.is_none() {
        let _ = repo.git.run(&[
            "branch",
            &format!("--set-upstream-to={remote}/{branch}", remote = home.name),
            &branch,
        ]);
    }

    if stashed {
        let pop = repo.git.run(&["stash", "pop", "--quiet"])?;
        if !pop.success() {
            // A failed pop can still leave the changes in the working tree with
            // conflict markers, and it keeps the stash entry in that case.
            if repo.status()?.conflicted() {
                ui.warning("Your unsaved changes overlap with the new saves.");
                ui.blank();
                ui.line("Nothing is lost. Let's sort them out one at a time.");
                ui.next(&[("meow resolve", "combine your unsaved work")]);
                record_conflict(&repo, &home.name, &branch, &label, head_before, snapshot);
                return Ok(1);
            }
            // Otherwise try the softer `apply`, which handles an untracked file
            // that the incoming saves introduced under the same name.
            let apply = repo.git.run(&["stash", "apply", "--quiet"])?;
            if apply.success() {
                if repo.status()?.conflicted() {
                    ui.warning("Your unsaved changes overlap with the new saves.");
                    ui.blank();
                    ui.line("Nothing is lost. Let's sort them out one at a time.");
                    ui.next(&[("meow resolve", "combine your unsaved work")]);
                    record_conflict(&repo, &home.name, &branch, &label, head_before, snapshot);
                    return Ok(1);
                }
                let _ = repo.git.run(&["stash", "drop", "--quiet", "stash@{0}"]);
                ui.success("Your unsaved changes are back in place.");
            } else {
                ui.warning("Your unsaved changes are still set aside safely.");
                ui.line("Nothing was lost — `meow doctor` shows where they are.");
                return Ok(1);
            }
        }
    }

    let incoming = match (&head_before, &head_after) {
        (Some(before), Some(after)) if before != after => repo
            .commits_between(before, after)
            .into_iter()
            .filter(|l| !l.contains("Merge"))
            .count(),
        _ => 0,
    };

    let mut journal = Journal::load(&repo.git_dir);
    let mut action = Action::new(
        "get",
        format!(
            "Got {incoming} new {} from {label}",
            if incoming == 1 { "save" } else { "saves" }
        ),
    );
    action.head_before = head_before.clone();
    action.head_after = head_after.clone();
    action.snapshot = Some(snapshot);
    action.data =
        json!({ "remote": home.name, "branch": branch, "count": incoming, "label": label });
    let _ = journal.record(&repo.git_dir, action);

    if ui.plain {
        if incoming == 0 {
            println!("up to date {branch}");
        } else {
            println!(
                "got {branch} {incoming} new {} from {label}",
                if incoming == 1 { "save" } else { "saves" }
            );
        }
        return Ok(0);
    }

    if incoming == 0 {
        ui.line(format!("{} You're up to date.", ui.sym(Sym::Success)));
        return Ok(0);
    }

    ui.success(format!(
        "Got {} new {}.",
        incoming,
        if incoming == 1 { "save" } else { "saves" }
    ));
    if let Some(after) = &head_after {
        if let Some(raw) = repo.git.probe(&[
            "log",
            "-1",
            "--no-merges",
            "--pretty=format:%s%x00%an%x00%aI",
            after,
        ]) {
            let mut f = raw.split('\0');
            let subject = f.next().unwrap_or_default();
            let author = f.next().unwrap_or_default();
            let when = f
                .next()
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok());
            if let Some(when) = when {
                ui.line(format!(
                    "  Last one: \"{subject}\" — {} · {}",
                    author_label(author, None),
                    relative_time(when)
                ));
            }
        }
    }
    if let (Some(before), Some(after)) = (&head_before, &head_after) {
        if let Some(raw) = repo
            .git
            .probe(&["diff", "--name-only", &format!("{before}..{after}")])
        {
            let files: Vec<&str> = raw.lines().filter(|l| !l.trim().is_empty()).collect();
            if !files.is_empty() {
                ui.line(format!(
                    "  {} {} changed{}",
                    files.len(),
                    if files.len() == 1 { "file" } else { "files" },
                    folder_hint(&files)
                ));
            }
        }
    }
    Ok(0)
}

fn folder_hint(files: &[&str]) -> String {
    let mut dirs = std::collections::BTreeSet::new();
    for file in files {
        if let Some((dir, _)) = file.split_once('/') {
            dirs.insert(dir.to_string());
        } else {
            return String::new();
        }
    }
    if dirs.len() == 1 {
        format!(" in {}/", dirs.into_iter().next().unwrap_or_default())
    } else {
        String::new()
    }
}

fn report_git(ui: &Ui, out: &crate::git::GitOutput, fallback: &str) -> Result<i32> {
    let err = GitError {
        stdout: out.stdout.clone(),
        stderr: out.stderr.clone(),
    };
    let failure = classify(&err);
    if failure == crate::git::Failure::Other {
        ui.failure(fallback);
        for line in out.stderr.lines().take(10) {
            ui.line(ui.dim(line));
        }
    } else {
        ui.failure(failure.message());
        ui.blank();
        ui.line(failure.hint());
    }
    Ok(1)
}

/// Record the attempt before returning for conflict help, so resolving the
/// overlap and then running `meow undo` returns to the pre-get state.
#[allow(clippy::too_many_arguments)]
fn record_conflict(
    repo: &Repo,
    remote: &str,
    branch: &str,
    label: &str,
    head_before: Option<String>,
    snapshot: String,
) {
    let mut action = Action::new("get", format!("Brought in saves from {label}"));
    action.head_before = head_before;
    action.head_after = repo.head();
    action.snapshot = Some(snapshot);
    action.data = json!({ "remote": remote, "branch": branch, "conflicted": true });
    let mut journal = Journal::load(&repo.git_dir);
    let _ = journal.record(&repo.git_dir, action);
}

fn is_empty(dir: &Path) -> bool {
    match fs::read_dir(dir) {
        Ok(mut entries) => entries.next().is_none(),
        Err(_) => false,
    }
}
