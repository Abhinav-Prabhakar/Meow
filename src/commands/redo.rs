//! `meow redo` — put back what you just undid.

use anyhow::Result;

use crate::journal::{Action, Journal};
use crate::ui::Ui;
use crate::util::require_repo;

pub fn run(ui: &Ui) -> Result<i32> {
    let repo = require_repo(ui)?;
    let mut journal = Journal::load_strict(&repo.git_dir)?;
    let Some(mut action) = journal.redo.pop() else {
        if ui.plain {
            println!("nothing to redo");
        } else {
            ui.line("There's nothing to redo.");
        }
        return Ok(0);
    };

    let result = redo_one(ui, &repo, &mut action);
    match result {
        Ok(Some(message)) => {
            journal.actions.push(action);
            journal.save(&repo.git_dir)?;
            if ui.plain {
                println!("redid {}", message);
            } else {
                ui.success(format!("Redid: {message}"));
            }
            Ok(0)
        }
        Ok(None) => {
            journal.redo.push(action);
            journal.save(&repo.git_dir)?;
            ui.failure("That can't be redone safely. Nothing was changed.");
            Ok(1)
        }
        Err(err) => {
            journal.redo.push(action);
            journal.save(&repo.git_dir)?;
            Err(err)
        }
    }
}

fn redo_one(ui: &Ui, repo: &crate::repo::Repo, action: &mut Action) -> Result<Option<String>> {
    match action.kind.as_str() {
        "save" => {
            let note = action
                .data
                .get("note")
                .and_then(|v| v.as_str())
                .unwrap_or(&action.summary)
                .to_string();
            if repo.head() != action.head_before {
                return Ok(None);
            }
            if !repo.status()?.dirty() {
                return Ok(None);
            }
            let _ = repo.git.run(&["add", "-A"]);
            let out = repo.git.run_main(&["commit", "-m", &note])?;
            if !out.success() {
                return Ok(None);
            }
            action.head_after = repo.head();
            let _ = ui;
            Ok(Some(format!("Saved \"{note}\"")))
        }
        "branch" => {
            let to = action
                .data
                .get("to")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            if to.is_empty() {
                return Ok(None);
            }
            let exists = repo.branches()?.iter().any(|b| b.name == to);
            if exists {
                repo.git.check(&["switch", &to])?;
            } else {
                repo.git.check(&["switch", "-c", &to])?;
            }
            Ok(Some(format!("Switched to {to}")))
        }
        "branch-delete" => {
            let name = action
                .data
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            if name.is_empty() {
                return Ok(None);
            }
            let exists = repo.branches()?.iter().any(|b| b.name == name);
            if !exists {
                return Ok(None);
            }
            repo.git.check(&["branch", "-D", &name])?;
            Ok(Some(format!("Deleted branch {name}")))
        }
        "ignore" => {
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
                return Ok(None);
            }
            for pattern in &patterns {
                crate::commands::save::add_ignore(repo, pattern)?;
            }
            Ok(Some("Ignored those files again".to_string()))
        }
        _ => Ok(None),
    }
}
