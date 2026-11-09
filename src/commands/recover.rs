//! `meow recover` — find work you thought you lost.

use std::fs;

use anyhow::Result;
use serde_json::json;

use crate::journal::{self, Action, Journal, SnapshotMeta, TrashMeta};
use crate::repo::Repo;
use crate::ui::{display_width, Ui};
use crate::util::{count, relative_to, require_repo};

enum Item {
    Snapshot(SnapshotMeta),
    Trash(TrashMeta),
}

pub fn run(ui: &Ui, purge: bool) -> Result<i32> {
    let repo = require_repo(ui)?;
    if purge {
        return purge_all(ui, &repo);
    }

    let mut items = Vec::new();
    for snapshot in journal::list_snapshots(&repo) {
        items.push(Item::Snapshot(snapshot));
    }
    for trash in journal::list_trash(&repo.git_dir) {
        items.push(Item::Trash(trash));
    }
    items.sort_by(|a, b| match (a, b) {
        (Item::Snapshot(a), Item::Snapshot(b)) => b.at.cmp(&a.at),
        (Item::Trash(a), Item::Trash(b)) => b.at.cmp(&a.at),
        (Item::Snapshot(a), Item::Trash(b)) => a.at.cmp(&b.at),
        (Item::Trash(a), Item::Snapshot(b)) => a.at.cmp(&b.at),
    });

    if items.is_empty() {
        if ui.plain {
            println!("recoverable 0");
        } else {
            ui.title("Recoverable work · none");
            ui.blank();
            ui.line("Nothing to recover right now.");
            ui.line(ui.dim("Meow keeps discarded work and safety snapshots here for 30 days."));
        }
        return Ok(0);
    }

    if ui.plain || !ui.can_prompt() {
        for (i, item) in items.iter().enumerate() {
            match item {
                Item::Snapshot(s) => println!(
                    "snapshot {} {} {} {}",
                    i + 1,
                    s.at.to_rfc3339(),
                    s.files,
                    s.label
                ),
                Item::Trash(t) => println!(
                    "trash {} {} {} {}",
                    i + 1,
                    t.at.to_rfc3339(),
                    t.files.len(),
                    t.label
                ),
            }
        }
        if ui.plain {
            return Ok(0);
        }
        return Err(ui.decision_required(
            "What would you like back?",
            "Run `meow recover` in a terminal to choose.",
        ));
    }

    ui.title("Recoverable work");
    ui.blank();
    let now = chrono::Utc::now();
    let labels: Vec<String> = items
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let (label, at, count_text) = match item {
                Item::Snapshot(s) => (&s.label, s.at, count(s.files, "file")),
                Item::Trash(t) => (&t.label, t.at, count(t.files.len(), "file")),
            };
            let pad = " ".repeat(
                ui.width()
                    .min(72)
                    .saturating_sub(display_width(label) + 10)
                    .max(2),
            );
            format!(
                "{:>2}  {label}{pad}{} · {count_text}",
                i + 1,
                relative_to(at, now)
            )
        })
        .collect();
    let picked = ui.select("What would you like back?", &labels, 0)?;
    match &items[picked] {
        Item::Snapshot(snapshot) => restore_snapshot(ui, &repo, snapshot),
        Item::Trash(trash) => restore_trash(ui, &repo, trash),
    }
}

fn restore_snapshot(ui: &Ui, repo: &Repo, snapshot: &SnapshotMeta) -> Result<i32> {
    let changed = changed_files(repo, &snapshot.sha);
    if changed.is_empty() {
        ui.line("That snapshot matches your current files — nothing to bring back.");
        return Ok(0);
    }
    ui.blank();
    ui.line(format!("From \"{}\":", snapshot.label));
    for path in changed.iter().take(8) {
        ui.line(format!("  {}", ui.dim(path)));
    }
    if changed.len() > 8 {
        ui.line(ui.dim(format!("  … and {} more", changed.len() - 8)));
    }
    ui.blank();
    ui.line("Your current version is kept first, so nothing is overwritten for good.");

    let mut chosen = changed.clone();
    if ui.can_prompt() {
        let labels: Vec<String> = changed.clone();
        let defaults = vec![true; labels.len()];
        let picked = ui.multi_select("Which files?", &labels, &defaults)?;
        chosen = picked
            .into_iter()
            .filter_map(|i| changed.get(i).cloned())
            .collect();
        if chosen.is_empty() {
            ui.line("Nothing was restored.");
            return Ok(0);
        }
        if !ui.confirm(
            &format!("Bring back {}?", count(chosen.len(), "file")),
            true,
        )? {
            ui.line("Nothing was restored.");
            return Ok(0);
        }
    }

    let safety = journal::create_snapshot(repo, "Before recovering an older version")?;
    let mut args: Vec<&str> = vec!["restore", "--source", &snapshot.sha, "--worktree", "--"];
    args.extend(chosen.iter().map(String::as_str));
    repo.git.check(&args)?;

    let mut action = Action::new(
        "recover",
        format!("Recovered {}", count(chosen.len(), "file")),
    );
    action.snapshot = Some(safety);
    action.data = json!({ "files": chosen, "from": snapshot.sha, "label": snapshot.label });
    let mut journal = Journal::load(&repo.git_dir);
    let _ = journal.record(&repo.git_dir, action);

    if ui.plain {
        println!("recovered {}", count(chosen.len(), "file"));
        return Ok(0);
    }
    ui.success(format!("Brought back {}.", count(chosen.len(), "file")));
    ui.next(&[("meow changes", "review what came back")]);
    Ok(0)
}

fn restore_trash(ui: &Ui, repo: &Repo, trash: &TrashMeta) -> Result<i32> {
    ui.blank();
    ui.line(format!("Discarded \"{}\":", trash.label));
    for path in trash.files.iter().take(8) {
        ui.line(format!("  {}", ui.dim(path)));
    }
    if trash.files.len() > 8 {
        ui.line(ui.dim(format!("  … and {} more", trash.files.len() - 8)));
    }
    ui.blank();
    ui.line("If a file exists again, the recovered copy gets a `.meow-recovered-` name.");
    if ui.can_prompt() && !ui.confirm("Bring it back?", true)? {
        ui.line("Nothing was restored.");
        return Ok(0);
    }

    let before = journal::create_snapshot(repo, "Before recovering discarded files")?;
    let restored = journal::restore_trash(repo, trash, None)?;
    let mut action = Action::new(
        "recover",
        format!("Recovered {}", count(restored.len(), "file")),
    );
    action.snapshot = Some(before);
    action.data = json!({ "files": restored, "trash": trash.id });
    let mut journal = Journal::load(&repo.git_dir);
    let _ = journal.record(&repo.git_dir, action);

    if ui.plain {
        println!("recovered {}", count(restored.len(), "file"));
        return Ok(0);
    }
    ui.success(format!("Brought back {}.", count(restored.len(), "file")));
    Ok(0)
}

fn purge_all(ui: &Ui, repo: &Repo) -> Result<i32> {
    let snapshots = journal::list_snapshots(repo);
    let trash = journal::list_trash(&repo.git_dir);
    if snapshots.is_empty() && trash.is_empty() {
        ui.line("There's nothing to purge.");
        return Ok(0);
    }
    ui.blank();
    ui.warning("This permanently removes Meow's local safety copies.");
    ui.line(format!(
        "{} and {} would be removed.",
        count(snapshots.len(), "snapshot"),
        count(trash.len(), "trash entry")
    ));
    ui.line("Your project's saves and files are not touched.");
    ui.blank();
    if ui.can_prompt() && !ui.confirm("Permanently remove them?", false)? {
        ui.line("Nothing was removed.");
        return Ok(0);
    }

    for snapshot in &snapshots {
        journal::remove_snapshot_ref(repo, &snapshot.id);
    }
    if let Ok(dir) = journal::ensure_meow_dir(&repo.git_dir) {
        let _ = fs::remove_file(dir.join("snapshots.json"));
    }
    let removed = journal::purge_trash(&repo.git_dir)?;

    if ui.plain {
        println!("purged {} snapshots {} trash", snapshots.len(), removed);
        return Ok(0);
    }
    ui.success(format!(
        "Removed {} and {}.",
        count(snapshots.len(), "snapshot"),
        count(removed, "trash entry")
    ));
    Ok(0)
}

fn changed_files(repo: &Repo, sha: &str) -> Vec<String> {
    let Some(raw) = repo.git.probe(&["diff", "--name-only", "-z", sha]) else {
        return Vec::new();
    };
    raw.split('\0')
        .filter(|p| !p.trim().is_empty())
        .map(str::to_string)
        .collect()
}
