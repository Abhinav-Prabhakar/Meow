//! `meow history` — past saves, grouped by day, with a way back.

use anyhow::Result;
use serde_json::json;

use crate::journal::{self, Action, Journal};
use crate::repo::{Commit, Repo};
use crate::ui::{display_width, Sym, Ui};
use crate::util::{author_label, day_label, relative_time, require_repo, truncate_chars};

pub fn run(
    ui: &Ui,
    n: Option<usize>,
    all: bool,
    long: bool,
    file: Option<&str>,
    restore: bool,
) -> Result<i32> {
    let repo = require_repo(ui)?;
    if !repo.has_commits() {
        if ui.plain {
            println!("history 0");
        } else {
            ui.title("History · empty");
            ui.blank();
            ui.line("No saves yet. Your first save will appear here.");
            ui.next(&[("meow save \"First save\"", "save your work")]);
        }
        return Ok(0);
    }

    match n {
        Some(position) => detail(ui, &repo, position, restore),
        None => {
            list(ui, &repo, all, long, file)?;
            Ok(0)
        }
    }
}

fn list(ui: &Ui, repo: &Repo, all: bool, long: bool, file: Option<&str>) -> Result<()> {
    let commits = repo.commits(10, all, file)?;
    let me = repo.identity().map(|(n, _)| n);
    let branch = repo.branch().unwrap_or_else(|| "main".to_string());

    if ui.plain {
        for commit in &commits {
            println!(
                "save {} {} \"{}\" {} {}",
                commit.short,
                branch,
                commit.subject,
                commit.author,
                commit.when.to_rfc3339()
            );
        }
        return Ok(());
    }

    ui.title(&format!(
        "History · {}{}",
        branch,
        if all { " · all branches" } else { "" }
    ));
    ui.blank();

    let mut current_day = String::new();
    for (i, commit) in commits.iter().enumerate() {
        let day = day_label(commit.when);
        if day != current_day {
            if !current_day.is_empty() {
                ui.blank();
            }
            ui.line(ui.bold(&day));
            current_day = day;
        }
        let dot = if i == 0 {
            ui.green(ui.sym(Sym::Dot))
        } else {
            ui.dim(ui.sym(Sym::Circle))
        };
        let subject = truncate_chars(&commit.subject, 44);
        let time = if long {
            commit.when.format("%Y-%m-%d %H:%M").to_string()
        } else {
            relative_time(commit.when)
        };
        let who = author_label(&commit.author, me.as_deref());
        let mut right = format!("{who} · {time}");
        if long {
            right = format!("{right} · {}", commit.short);
        }
        let width = ui.width().min(76);
        let left_plain = subject.to_string();
        let gap = width.saturating_sub(7 + display_width(&left_plain) + display_width(&right));
        if gap >= 2 {
            ui.line(format!(
                "  {dot} {left_plain}{}{}",
                " ".repeat(gap),
                ui.dim(right)
            ));
        } else {
            ui.line(format!("  {dot} {left_plain}  {}", ui.dim(right)));
        }
    }

    let status = repo.status()?;
    let unsent = crate::commands::home::unsent_count(repo, &status);
    if unsent > 0 {
        ui.blank();
        ui.line(ui.dim(format!("{} {} not sent", ui.sym(Sym::Up), unsent)));
    }
    if commits.len() < 10 {
        ui.blank();
    }
    Ok(())
}

fn detail(ui: &Ui, repo: &Repo, position: usize, restore: bool) -> Result<i32> {
    let commits = repo.commits(position.max(1), false, None)?;
    let Some(commit) = commits.get(position.saturating_sub(1)) else {
        ui.line(format!("There is no save #{position}."));
        return Ok(0);
    };
    let branch = repo.branch().unwrap_or_else(|| "main".to_string());

    if ui.plain {
        println!("save {} \"{}\"", commit.short, commit.subject);
        for (status, path) in commit_status(repo, &commit.sha) {
            println!("{status} {path}");
        }
        if restore {
            restore_version(ui, repo, commit, position)?;
        }
        return Ok(0);
    }

    ui.title(&format!("Save #{} · \"{}\"", position, commit.subject));
    ui.blank();
    let me = repo.identity().map(|(n, _)| n);
    ui.line(ui.dim(format!(
        "{} · {} · {}",
        author_label(&commit.author, me.as_deref()),
        relative_time(commit.when),
        branch
    )));
    ui.blank();
    let stats = repo.commit_stat(&commit.sha);
    ui.line(format!(
        "{} changed · +{} -{}",
        crate::util::count(stats.files, "file"),
        stats.added,
        stats.deleted
    ));
    for (change, path) in commit_status(repo, &commit.sha) {
        ui.line(format!("  {}  {}", change, path));
    }
    ui.blank();

    if restore {
        restore_version(ui, repo, commit, position)?;
        return Ok(0);
    }

    if !ui.can_prompt() {
        return Ok(0);
    }
    let choice = ui.select(
        "What next?",
        &[
            "See the changes".to_string(),
            "Bring this version back".to_string(),
            "Back".to_string(),
        ],
        0,
    )?;
    match choice {
        0 => {
            let out = repo.git.check(&[
                "show",
                "--format=",
                "-M",
                "--unified=3",
                if ui.color {
                    "--color=always"
                } else {
                    "--color=never"
                },
                &commit.sha,
            ])?;
            ui.page(&out.stdout);
            ui.blank();
        }
        1 => restore_version(ui, repo, commit, position)?,
        _ => {}
    }
    Ok(0)
}

fn restore_version(ui: &Ui, repo: &Repo, commit: &Commit, position: usize) -> Result<()> {
    let changes = commit_status(repo, &commit.sha);
    let mut to_remove = Vec::new();
    let mut to_restore = Vec::new();
    for (code, path) in &changes {
        if code == "D" {
            to_remove.push(path.clone());
        } else {
            to_restore.push(path.clone());
        }
    }
    if to_restore.is_empty() && to_remove.is_empty() {
        ui.line("That save didn't change any files.");
        return Ok(());
    }

    let snapshot =
        journal::create_snapshot(repo, &format!("Before bringing back save #{position}"))?;
    if !to_restore.is_empty() {
        let mut args: Vec<&str> = vec!["restore", "--source", &commit.sha, "--worktree", "--"];
        args.extend(to_restore.iter().map(String::as_str));
        repo.git.check(&args)?;
    }
    for path in &to_remove {
        let full = repo.root.join(path);
        if full.exists() {
            let _ = journal::move_to_trash(
                repo,
                std::slice::from_ref(path),
                "Replaced by an older version",
            );
        }
    }

    let mut action = Action::new(
        "restore",
        format!("Bring back save #{} \"{}\"", position, commit.subject),
    );
    action.head_before = repo.head();
    action.head_after = repo.head();
    action.snapshot = Some(snapshot);
    action.data = json!({
        "files": to_restore,
        "removed": to_remove,
        "save": commit.sha,
    });
    let mut journal = Journal::load(&repo.git_dir);
    let _ = journal.record(&repo.git_dir, action);

    ui.success(format!(
        "Brought save #{} back as unsaved changes.",
        position
    ));
    ui.line(ui.dim("Nothing was overwritten for good — the previous state is in `meow recover`."));
    ui.next(&[
        ("meow changes", "review what came back"),
        (
            "meow save \"Bring back an older version\"",
            "save it as a new checkpoint",
        ),
    ]);
    Ok(())
}

/// Per-file status letters for a commit, as (letter, path).
fn commit_status(repo: &Repo, sha: &str) -> Vec<(String, String)> {
    let Some(raw) = repo
        .git
        .probe(&["show", "--name-status", "--format=", "-z", sha])
    else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut it = raw.split('\0');
    while let Some(status) = it.next() {
        if status.trim().is_empty() {
            continue;
        }
        let code = status.chars().next().unwrap_or('M').to_string();
        let Some(path) = it.next() else { break };
        if path.is_empty() {
            continue;
        }
        if code == "R" || code == "C" {
            // Rename/copy records carry the old path before the new one.
            if let Some(new_path) = it.next() {
                out.push((code.clone(), new_path.to_string()));
                continue;
            }
        }
        out.push((code, path.to_string()));
    }
    out
}
