//! `meow undo` — take back the last action, using the local journal.

use anyhow::Result;

use crate::journal::{self, Action, Journal};
use crate::repo::Repo;
use crate::ui::Ui;
use crate::util::require_repo;

enum Outcome {
    Done(String),
    Refused(String),
}

pub fn run(ui: &Ui, n: Option<usize>, list: bool) -> Result<i32> {
    let repo = require_repo(ui)?;
    let mut journal = Journal::load_strict(&repo.git_dir)?;

    if list {
        return show_list(ui, &journal);
    }

    if journal.actions.is_empty() {
        if ui.plain {
            println!("nothing to undo");
        } else {
            ui.line("There's nothing to undo yet.");
        }
        return Ok(0);
    }

    let count = n.unwrap_or(1).max(1);
    let mut done = 0;
    while done < count {
        let Some(action) = journal.actions.pop() else {
            break;
        };
        match undo_one(ui, &repo, &action) {
            Ok(Outcome::Done(message)) => {
                if ui.plain {
                    println!("undid {} {}", action.kind, action.summary);
                } else {
                    ui.success(format!("Undid: {}", action.summary));
                    if !message.is_empty() {
                        ui.line(ui.dim(&message));
                    }
                }
                journal.redo.push(action);
                done += 1;
            }
            Ok(Outcome::Refused(message)) => {
                journal.actions.push(action);
                journal.save(&repo.git_dir)?;
                ui.failure(message);
                return Ok(1);
            }
            Err(err) => {
                journal.actions.push(action);
                journal.save(&repo.git_dir)?;
                return Err(err);
            }
        }
    }
    journal.save(&repo.git_dir)?;

    if done == 0 {
        ui.line("Nothing changed.");
        return Ok(0);
    }
    if !ui.plain {
        ui.blank();
        ui.line(ui.dim(format!("Changed your mind?  {}", ui.cyan("meow redo"))));
    }
    Ok(0)
}

fn show_list(ui: &Ui, journal: &Journal) -> Result<i32> {
    if ui.plain {
        for action in journal.actions.iter().rev() {
            println!(
                "action {} {} {}",
                action.at.to_rfc3339(),
                action.kind,
                action.summary
            );
        }
        return Ok(0);
    }
    let actions: Vec<_> = journal.actions.iter().rev().take(10).collect();
    if actions.is_empty() {
        ui.line("No recent actions.");
        return Ok(0);
    }
    ui.title("Recent actions");
    ui.blank();
    for action in actions {
        let undoable = if action.kind == "send" {
            ui.dim("can't be undone")
        } else {
            ui.green("undoable")
        };
        ui.line(format!(
            "  {}  {}{}{}",
            action.at.format("%H:%M"),
            action.summary,
            " ".repeat(2),
            undoable
        ));
    }
    Ok(0)
}

fn undo_one(ui: &Ui, repo: &Repo, action: &Action) -> Result<Outcome> {
    match action.kind.as_str() {
        "save" => undo_save(ui, repo, action),
        "discard" => undo_discard(repo, action),
        "get" | "merge" => undo_sync(repo, action),
        "branch" => undo_branch(repo, action),
        "branch-delete" => undo_branch_delete(repo, action),
        "ignore" => undo_ignore(repo, action),
        "restore" => undo_restore(repo, action),
        "send" => Ok(Outcome::Refused(
            "Sending can't be undone — the work is already online. Your local copy is safe."
                .to_string(),
        )),
        other => Ok(Outcome::Refused(format!(
            "Meow doesn't know how to undo a '{other}' action. Nothing was changed."
        ))),
    }
}

fn undo_save(ui: &Ui, repo: &Repo, action: &Action) -> Result<Outcome> {
    let current = repo.head();
    if current != action.head_after {
        return Ok(Outcome::Refused(
            "The project changed after that save, so Meow won't guess. Nothing was changed."
                .to_string(),
        ));
    }
    if let Some(head_after) = &action.head_after {
        if is_sent(repo, head_after) {
            return undo_sent(ui, repo, action);
        }
    }

    let note = action
        .data
        .get("note")
        .and_then(|v| v.as_str())
        .unwrap_or(&action.summary);

    let Some(before) = &action.head_before else {
        // Undoing the very first save: keep the files, remove the checkpoint.
        repo.git.check(&["update-ref", "-d", "HEAD"])?;
        let _ = repo.git.run(&["read-tree", "--empty"]);
        return Ok(Outcome::Done(
            "Your work is still here — it's just not saved anymore.".to_string(),
        ));
    };

    if !ui.can_prompt() {
        repo.git.check(&["reset", before])?;
        return Ok(Outcome::Done(format!(
            "Your changes are still in your files. \"{note}\" is no longer a save."
        )));
    }

    ui.blank();
    ui.line(format!("Your last save was \"{note}\"."));
    ui.line("It hasn't been sent yet, so it's safe to undo.");
    ui.blank();
    let choice = ui.select(
        "What should happen to your changes?",
        &[
            "Keep them in my files (recommended)".to_string(),
            "Remove them too".to_string(),
        ],
        0,
    )?;
    if choice == 0 {
        repo.git.check(&["reset", before])?;
        Ok(Outcome::Done(
            "Your work is still here — it's just not saved anymore.".to_string(),
        ))
    } else {
        let snapshot =
            journal::create_snapshot(repo, "Before undoing a save and removing its changes")?;
        repo.git.check(&["reset", "--hard", before])?;
        let mut journal = Journal::load(&repo.git_dir);
        let mut action = Action::new("undo-save-remove", "Removed a save and its changes");
        action.snapshot = Some(snapshot);
        let _ = journal.record(&repo.git_dir, action);
        Ok(Outcome::Done(
            "The changes were removed. `meow recover` can bring them back.".to_string(),
        ))
    }
}

fn undo_sent(ui: &Ui, repo: &Repo, action: &Action) -> Result<Outcome> {
    let note = action
        .data
        .get("note")
        .and_then(|v| v.as_str())
        .unwrap_or(&action.summary);
    if repo.status()?.dirty() {
        return Ok(Outcome::Refused(
            "That save is already shared. Save or discard your current changes first, then Meow can reverse it safely."
                .to_string(),
        ));
    }
    if !repo.has_commits() {
        return Ok(Outcome::Refused("There's nothing to reverse.".to_string()));
    }
    if !ui.can_prompt() {
        return Ok(Outcome::Refused(
            "That save is already shared; Meow won't rewrite history in a script.".to_string(),
        ));
    }
    ui.blank();
    ui.line("That save is already shared, so Meow won't rewrite history.");
    ui.line("Instead, it can make a new save that reverses it — safe for everyone.");
    ui.blank();
    if !ui.confirm(&format!("Create \"Undo: {note}\"?"), false)? {
        return Ok(Outcome::Refused("Nothing was changed.".to_string()));
    }
    let out = repo.git.run_main(&["revert", "--no-edit", "HEAD"])?;
    if !out.success() {
        let err = anyhow::anyhow!("{}", out.stderr.trim());
        return Ok(Outcome::Refused(format!(
            "The reversal couldn't be made: {err}"
        )));
    }
    Ok(Outcome::Done(
        "A new save now reverses the old one. Everyone stays in sync.".to_string(),
    ))
}

fn undo_discard(repo: &Repo, action: &Action) -> Result<Outcome> {
    if let Some(snapshot) = &action.snapshot {
        journal::restore_snapshot(repo, snapshot, false)?;
    }
    if let Some(ids) = action.data.get("trash").and_then(|v| v.as_array()) {
        for id in ids {
            if let Some(id) = id.as_str() {
                for meta in journal::list_trash(&repo.git_dir) {
                    if meta.id == id {
                        let _ = journal::restore_trash(repo, &meta, None);
                    }
                }
            }
        }
    }
    Ok(Outcome::Done(
        "Your discarded work is back in the project.".to_string(),
    ))
}

fn undo_sync(repo: &Repo, action: &Action) -> Result<Outcome> {
    let Some(before) = &action.head_before else {
        return Ok(Outcome::Refused(
            "Meow can't tell what this project looked like before that action.".to_string(),
        ));
    };
    if let Some(snapshot) = &action.snapshot {
        // Put the branch back first, then restore the working tree that
        // existed before the operation.
        let current = repo.head().unwrap_or_default();
        if &current != before {
            repo.git.check(&["reset", "--hard", before])?;
        }
        journal::restore_snapshot(repo, snapshot, true)?;
    } else {
        repo.git.check(&["reset", "--hard", before])?;
    }
    Ok(Outcome::Done(
        "Back to exactly where you were before.".to_string(),
    ))
}

fn undo_branch(repo: &Repo, action: &Action) -> Result<Outcome> {
    let from = action
        .data
        .get("from")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    let to = action
        .data
        .get("to")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    let created = action
        .data
        .get("created")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    if from.is_empty() || to.is_empty() {
        return Ok(Outcome::Refused(
            "Meow can't tell which branch you came from.".to_string(),
        ));
    }
    if repo.branch().as_deref() != Some(to) {
        return Ok(Outcome::Refused(format!(
            "You're not on {to} anymore, so nothing was changed."
        )));
    }
    repo.git.check(&["switch", from])?;
    if created {
        let _ = repo.git.run(&["branch", "-D", to]);
    }
    Ok(Outcome::Done(format!("Back on {from}.")))
}

fn undo_branch_delete(repo: &Repo, action: &Action) -> Result<Outcome> {
    let name = action
        .data
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    let sha = action
        .data
        .get("sha")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    if name.is_empty() || sha.is_empty() {
        return Ok(Outcome::Refused(
            "Meow can't tell which branch to bring back.".to_string(),
        ));
    }
    let exists = repo.branches()?.iter().any(|b| b.name == name);
    if exists {
        return Ok(Outcome::Refused(format!(
            "A branch called {name} exists again, so nothing was changed."
        )));
    }
    repo.git.check(&["branch", name, sha])?;
    Ok(Outcome::Done(format!("Brought back branch {name}.")))
}

fn undo_ignore(repo: &Repo, action: &Action) -> Result<Outcome> {
    let patterns: Vec<String> = action
        .data
        .get("patterns")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    if patterns.is_empty() {
        return Ok(Outcome::Refused("Nothing to restore.".to_string()));
    }
    let ignore = repo.root.join(".gitignore");
    let mut body = std::fs::read_to_string(&ignore).unwrap_or_default();
    let mut lines: Vec<String> = body.lines().map(str::to_string).collect();
    lines.retain(|l| !patterns.iter().any(|p| l.trim() == p.trim()));
    body = lines.join("\n");
    if !body.is_empty() {
        body.push('\n');
    }
    std::fs::write(&ignore, body)?;
    if let Some(tracked) = action.data.get("tracked").and_then(|v| v.as_array()) {
        let paths: Vec<&str> = tracked.iter().filter_map(|v| v.as_str()).collect();
        if !paths.is_empty() {
            let mut args: Vec<&str> = vec!["add", "--"];
            args.extend(paths);
            let _ = repo.git.run(&args);
        }
    }
    Ok(Outcome::Done("Those files are tracked again.".to_string()))
}

fn undo_restore(repo: &Repo, action: &Action) -> Result<Outcome> {
    if let Some(snapshot) = &action.snapshot {
        journal::restore_snapshot(repo, snapshot, true)?;
        Ok(Outcome::Done(
            "The project is back to how it was before the older version was brought back."
                .to_string(),
        ))
    } else {
        Ok(Outcome::Refused(
            "Meow can't find the snapshot for that action.".to_string(),
        ))
    }
}

fn is_sent(repo: &Repo, sha: &str) -> bool {
    repo.git
        .probe(&["branch", "-r", "--contains", sha])
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false)
}
