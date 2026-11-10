//! `meow` — where am I, what is my work doing, and what should I do next?

use std::fs;
use std::path::PathBuf;

use anyhow::Result;

use crate::repo::{Operation, Repo, Status};
use crate::ui::{display_width, Sym, Ui};
use crate::util::{display_path, suggest_note};

pub fn run(ui: &Ui) -> Result<i32> {
    let cwd = std::env::current_dir()?;
    match Repo::discover(&cwd, ui)? {
        Some(repo) => home(ui, &repo)?,
        None => welcome(ui)?,
    }
    Ok(0)
}

fn welcome(ui: &Ui) -> Result<()> {
    let first = mark_first_run();
    if ui.plain {
        println!("no project");
        println!("next: meow new");
        return Ok(());
    }
    ui.blank();
    if first {
        ui.line(format!("{}  meow", ui.sym(Sym::Cat)));
        ui.blank();
        ui.line(ui.dim("Save, share, and protect your work — without learning Git first."));
        ui.blank();
    } else {
        ui.line(ui.bold("meow"));
        ui.blank();
    }
    ui.line("This folder isn't a project yet.");
    ui.blank();
    table(
        ui,
        &[
            ("Start something new", "meow new"),
            ("Get a project from a link", "meow get <link>"),
            ("Practice safely", "meow learn"),
        ],
    );
    ui.blank();
    Ok(())
}

fn home(ui: &Ui, repo: &Repo) -> Result<()> {
    let status = repo.status()?;
    let path = display_path(&repo.root);

    if ui.plain {
        plain_home(ui, repo, &status, &path);
        return Ok(());
    }

    ui.header(&path);

    // In-progress work takes priority over everything else.
    if let Some(op) = status.operation {
        match op {
            Operation::Merge => {
                let other = merge_source(repo).unwrap_or_else(|| "another branch".to_string());
                let current = status
                    .branch
                    .clone()
                    .unwrap_or_else(|| "this branch".to_string());
                ui.line(format!(
                    "You're combining {} into {}.",
                    ui.cyan(other),
                    ui.cyan(current)
                ));
                if status.conflicted() {
                    ui.line(format!(
                        "{} {} your help.",
                        plural_files(status.conflicts.len()),
                        if status.conflicts.len() == 1 {
                            "still needs"
                        } else {
                            "still need"
                        }
                    ));
                } else {
                    ui.line("Everything is combined. Finish when you're ready.");
                }
                ui.next(&[
                    ("meow resolve", "finish the combine"),
                    ("meow cancel", "stop and go back"),
                ]);
                return Ok(());
            }
            Operation::Rebase | Operation::CherryPick | Operation::Revert | Operation::Bisect => {
                ui.line(format!(
                    "This project is in the middle of a Git {}.",
                    op.label()
                ));
                ui.line(ui.dim(format!(
                    "Meow doesn't manage {}; finish it with Git, or ask for help.",
                    op.label()
                )));
                ui.next(&[
                    ("meow help --git", "what this is called in Git"),
                    ("meow doctor", "check the project"),
                ]);
                return Ok(());
            }
        }
    }

    // Where am I?
    if status.detached {
        ui.line("You're looking at an old save, not a branch.");
        let back = format!("meow branch {}", repo.suggested_branch());
        ui.next(&[(back.as_str(), "go back to your branch")]);
        return Ok(());
    }
    let branch = status.branch.clone().unwrap_or_else(|| "main".to_string());
    ui.line(format!("You're on {}.", ui.bold(&branch)));
    if let Some(home) = repo.home_label() {
        ui.line(format!("{}  {}", ui.sym(Sym::Home), ui.dim(home)));
    }

    ui.blank();

    // What is the state of my work?
    let stat = repo.diff_stat();
    if status.dirty() {
        let label = format!(
            "{} {}",
            ui.green(ui.sym(Sym::Dot)),
            plural_changes(status.files.len())
        );
        if ui.width() >= 52 && stat.files > 0 {
            let right = format!(
                "{} {}",
                ui.green(format!("+{}", stat.added)),
                ui.red(format!("-{}", stat.deleted))
            );
            let gap = ui
                .width()
                .saturating_sub(4 + display_width(&label) + display_width(&right));
            ui.line(format!("{label}{}{right}", " ".repeat(gap.max(2))));
        } else {
            ui.line(label);
        }
    } else if repo.has_commits() {
        ui.line(format!("{} No unsaved changes.", ui.sym(Sym::Success)));
    } else {
        ui.line(ui.dim(format!("{} No saves yet.", ui.sym(Sym::Circle))));
    }

    let unsent = unsent_count(repo, &status);
    let incoming = incoming_count(repo, &status);
    if unsent > 0 {
        ui.line(format!(
            "{} {}",
            ui.cyan(ui.sym(Sym::Up)),
            plural_saves(unsent, "not sent yet")
        ));
    }
    if incoming > 0 {
        ui.line(format!(
            "{} {}",
            ui.cyan(ui.sym(Sym::Down)),
            plural_incoming(incoming)
        ));
    }

    // What should I do next?
    let mut next: Vec<(&str, &str)> = Vec::new();
    let mut save_hint = String::new();
    if status.dirty() {
        let files = status.files.clone();
        save_hint = format!("meow save \"{}\"", suggest_note(&files));
    }
    if !status.dirty() && !repo.has_commits() {
        next.push(("meow save \"First save\"", "save your files"));
    } else if status.dirty() {
        next.push((save_hint.as_str(), "save your changes"));
    }
    if incoming > 0 {
        next.push(("meow get", "get their work"));
    } else if unsent > 0 && repo.home_label().is_some() {
        next.push(("meow send", "put your saves online"));
    }
    if repo.home_label().is_none() && repo.has_commits() {
        next.push(("meow send", "put your project online"));
    }
    if next.is_empty() {
        if repo.has_commits() {
            next.push(("meow changes", "see what changed"));
            next.push(("meow history", "see past saves"));
        } else {
            next.push(("meow new", "start a project here"));
        }
    }
    next.truncate(2);
    if !next.is_empty() {
        ui.next(&next);
    }

    ui.blank();
    ui.hint("meow help for all commands");
    Ok(())
}

fn plain_home(ui: &Ui, repo: &Repo, status: &Status, path: &str) {
    println!("meow {path}");
    if status.detached {
        println!("branch (detached)");
        let stat = repo.diff_stat();
        println!(
            "unsaved {} +{} -{}",
            status.files.len(),
            stat.added,
            stat.deleted
        );
        println!("next: meow branch {}", repo.suggested_branch());
        return;
    }
    let branch = status.branch.clone().unwrap_or_else(|| "main".to_string());
    println!("branch {branch}");
    if let Some(home) = repo.home_label() {
        println!("home {home}");
    }
    let stat = repo.diff_stat();
    println!(
        "unsaved {} +{} -{}",
        status.files.len(),
        stat.added,
        stat.deleted
    );
    println!("unsent {}", unsent_count(repo, status));
    println!("incoming {}", incoming_count(repo, status));
    if status.conflicted() {
        println!("conflicts {}", plural_files(status.conflicts.len()));
    }
    let files = status.files.clone();
    if status.dirty() {
        println!("next: meow save \"{}\"", suggest_note(&files));
    } else if unsent_count(repo, status) > 0 {
        println!("next: meow send");
    } else if incoming_count(repo, status) > 0 {
        println!("next: meow get");
    } else {
        let _ = ui;
    }
}

pub fn unsent_count(repo: &Repo, status: &Status) -> usize {
    if status.upstream.is_some() {
        return status.ahead;
    }
    let Some(home) = repo.home_remote() else {
        return 0;
    };
    let Some(branch) = &status.branch else {
        return 0;
    };
    repo.git
        .probe(&[
            "rev-list",
            "--count",
            &format!("refs/remotes/{}/{}..HEAD", home.name, branch),
        ])
        .and_then(|s| s.parse().ok())
        .unwrap_or(0)
}

pub fn incoming_count(repo: &Repo, status: &Status) -> usize {
    if status.upstream.is_some() {
        return status.behind;
    }
    let Some(home) = repo.home_remote() else {
        return 0;
    };
    let Some(branch) = &status.branch else {
        return 0;
    };
    repo.git
        .probe(&[
            "rev-list",
            "--count",
            &format!("HEAD..refs/remotes/{}/{}", home.name, branch),
        ])
        .and_then(|s| s.parse().ok())
        .unwrap_or(0)
}

fn plural_changes(n: usize) -> String {
    if n == 1 {
        "1 unsaved change".to_string()
    } else {
        format!("{n} unsaved changes")
    }
}

fn plural_files(n: usize) -> String {
    if n == 1 {
        "1 file".to_string()
    } else {
        format!("{n} files")
    }
}

fn plural_saves(n: usize, tail: &str) -> String {
    if n == 1 {
        format!("1 save {tail}")
    } else {
        format!("{n} saves {tail}")
    }
}

fn plural_incoming(n: usize) -> String {
    if n == 1 {
        "1 new save from your team".to_string()
    } else {
        format!("{n} new saves from your team")
    }
}

fn table(ui: &Ui, rows: &[(&str, &str)]) {
    let width = rows
        .iter()
        .map(|(label, _)| display_width(label))
        .max()
        .unwrap_or(0)
        + 4;
    for (label, cmd) in rows {
        let pad = width.saturating_sub(display_width(label));
        ui.line(format!("{}{}{}", label, " ".repeat(pad), ui.cyan(cmd)));
    }
}

fn merge_source(repo: &Repo) -> Option<String> {
    let git_dir = &repo.git_dir;
    for name in ["MERGE_MSG", "rebase-merge/head-name"] {
        if let Ok(text) = fs::read_to_string(git_dir.join(name)) {
            let first = text.lines().next().unwrap_or("");
            if let Some(start) = first.find('\'') {
                if let Some(end) = first[start + 1..].find('\'') {
                    return Some(first[start + 1..start + 1 + end].to_string());
                }
            }
        }
    }
    None
}

fn mark_first_run() -> bool {
    let dir = dirs::state_dir()
        .or_else(dirs::config_dir)
        .map(|d| d.join("meow"));
    let Some(dir) = dir else {
        return false;
    };
    let marker: PathBuf = dir.join("welcome-seen");
    if marker.exists() {
        return false;
    }
    let _ = fs::create_dir_all(&dir);
    let _ = fs::write(&marker, b"1");
    true
}
