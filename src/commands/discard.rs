//! `meow discard` — throw away unsaved changes, safely.

use anyhow::Result;
use serde_json::json;

use crate::journal::{self, Action, Journal};
use crate::repo::{ChangeKind, FileChange, Status};
use crate::ui::Ui;
use crate::util::{count, require_repo};

pub fn run(ui: &Ui, paths: &[String], all: bool) -> Result<i32> {
    let repo = require_repo(ui)?;
    let status = repo.status()?;
    if status.conflicted() {
        ui.failure("Some files are mid-combine and need resolving, not discarding.");
        ui.next(&[
            ("meow resolve", "fix them one at a time"),
            ("meow cancel", "stop the combine instead"),
        ]);
        return Ok(1);
    }
    if status.files.is_empty() {
        if ui.plain {
            println!("discarded 0");
        } else {
            ui.line("There are no unsaved changes to throw away.");
        }
        return Ok(0);
    }

    let selected = if all {
        status.files.clone()
    } else if !paths.is_empty() {
        let matching = filter(&status, paths);
        if matching.is_empty() {
            ui.line("None of those files have unsaved changes.");
            return Ok(0);
        }
        matching
    } else {
        if !ui.can_prompt() {
            return Err(ui.decision_required(
                "Which changes should be thrown away?",
                "List the paths, or pass --all.",
            ));
        }
        choose(ui, &status)?
    };

    if selected.is_empty() {
        ui.line("Nothing was thrown away.");
        return Ok(0);
    }

    if ui.plain && paths.is_empty() && !all {
        return Err(ui.decision_required("Nothing was selected.", "List the paths, or pass --all."));
    }

    if !ui.plain {
        ui.blank();
        ui.line("These files will go back to how they were at your last save:");
        for file in &selected {
            ui.line(format!("  {}  {}", ui.dim(file.code()), file.path));
        }
        ui.blank();
        if selected.iter().any(|f| f.is_untracked()) {
            ui.line("Untracked files are moved to Meow's trash, not deleted.");
        }
        ui.line(ui.dim("You can bring any of this back for 30 days with `meow recover`."));
        ui.blank();
    }
    if ui.can_prompt()
        && !ui.confirm(
            &format!("Throw away {}?", count(selected.len(), "change")),
            false,
        )?
    {
        ui.line("Nothing was thrown away.");
        return Ok(0);
    }

    let snapshot = journal::create_snapshot(&repo, "Before discarding unsaved changes")?;
    let mut trashed = Vec::new();
    let mut restored = Vec::new();
    let mut removed = Vec::new();

    for file in &selected {
        if file.kind == ChangeKind::Untracked {
            let meta =
                journal::move_to_trash(&repo, std::slice::from_ref(&file.path), "Discarded")?;
            trashed.push(meta.id);
            continue;
        }
        // Tracked: put the file back the way it was at the last save.
        let out = repo.git.run(&[
            "restore",
            "--source",
            "HEAD",
            "--staged",
            "--worktree",
            "--",
            &file.path,
        ]);
        if out.map(|o| o.success()).unwrap_or(false) {
            restored.push(file.path.clone());
            continue;
        }
        if file.kind == ChangeKind::Added {
            // A new staged file: remove it from the index, then trash the file.
            let _ = repo.git.run(&["rm", "--cached", "--", &file.path]);
            if repo.root.join(&file.path).exists() {
                let meta =
                    journal::move_to_trash(&repo, std::slice::from_ref(&file.path), "Discarded")?;
                trashed.push(meta.id);
            } else {
                removed.push(file.path.clone());
            }
        } else {
            let full = repo.root.join(&file.path);
            if full.exists() {
                let _ =
                    journal::move_to_trash(&repo, std::slice::from_ref(&file.path), "Discarded");
            }
            removed.push(file.path.clone());
        }
    }

    let mut action = Action::new(
        "discard",
        format!("Threw away {}", count(selected.len(), "change")),
    );
    action.snapshot = Some(snapshot);
    action.data = json!({
        "paths": selected.iter().map(|f| f.path.clone()).collect::<Vec<_>>(),
        "trash": trashed,
        "restored": restored,
        "removed": removed,
    });
    let mut journal = Journal::load(&repo.git_dir);
    let _ = journal.record(&repo.git_dir, action);

    if ui.plain {
        // One path per line keeps output parseable even when names contain spaces.
        println!("discarded {}", selected.len());
        for file in &selected {
            println!("path {}", file.path);
        }
        return Ok(0);
    }
    ui.success(format!("Threw away {}.", count(selected.len(), "change")));
    ui.line(ui.dim("`meow recover` can bring it back if you change your mind."));
    let after = repo.status()?;
    if after.dirty() {
        ui.blank();
        ui.next(&[("meow changes", "see what's still unsaved")]);
    }
    Ok(0)
}

fn choose(ui: &Ui, status: &Status) -> Result<Vec<FileChange>> {
    let labels: Vec<String> = status
        .files
        .iter()
        .map(|f| format!("{}  {}", f.code(), f.path))
        .collect();
    let defaults = vec![false; labels.len()];
    let picked = ui.multi_select("Which changes should be thrown away?", &labels, &defaults)?;
    Ok(picked
        .into_iter()
        .filter_map(|i| status.files.get(i).cloned())
        .collect())
}

fn filter(status: &Status, paths: &[String]) -> Vec<FileChange> {
    status
        .files
        .iter()
        .filter(|f| {
            paths.iter().any(|p| {
                f.path == *p || f.path.starts_with(&format!("{}/", p.trim_end_matches('/')))
            })
        })
        .cloned()
        .collect()
}
