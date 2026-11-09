//! `meow branch` — list, start, switch, or delete a parallel version.

use anyhow::Result;
use serde_json::json;

use crate::journal::{self, Action, Journal};
use crate::repo::Repo;
use crate::ui::{display_width, Sym, Ui};
use crate::util::{relative_time, require_repo, slugify};

pub fn run(ui: &Ui, args: &[String], delete_flag: bool, force: bool) -> Result<i32> {
    let repo = require_repo(ui)?;

    // `meow branch delete <name>` reads naturally; accept it as well as --delete.
    let (delete, name) = if delete_flag {
        (true, args.first().map(String::as_str))
    } else if args.first().map(String::as_str) == Some("delete") {
        (true, args.get(1).map(String::as_str))
    } else if args.first().map(String::as_str) == Some("list") {
        (false, None)
    } else {
        (false, args.first().map(String::as_str))
    };

    if delete {
        return delete_branch(ui, &repo, name, force);
    }
    match name {
        Some(name) => switch_or_create(ui, &repo, name),
        None => list(ui, &repo),
    }
}

fn list(ui: &Ui, repo: &Repo) -> Result<i32> {
    let branches = repo.branches()?;
    let current = repo.branch().unwrap_or_default();
    if ui.plain {
        for branch in &branches {
            println!(
                "branch {} {} {}",
                if branch.name == current { "*" } else { " " },
                branch.name,
                if branch.merged { "merged" } else { "open" }
            );
        }
        return Ok(0);
    }

    ui.title("Branches");
    ui.blank();
    for branch in &branches {
        let here = branch.name == current;
        let dot = if here {
            ui.green(ui.sym(Sym::Dot))
        } else {
            ui.dim(ui.sym(Sym::Circle))
        };
        let count = repo
            .git
            .probe(&["rev-list", "--count", &branch.name])
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(0);
        let mut tail = format!("{count} {}", if count == 1 { "save" } else { "saves" });
        if here {
            tail = format!(
                "you're here · {count} {}",
                if count == 1 { "save" } else { "saves" }
            );
        } else if branch.merged {
            tail = format!("{tail} · fully combined into {current}");
        } else if let Some(when) = branch.when {
            tail = format!("{tail} · last used {}", relative_time(when));
        }
        let name = &branch.name;
        let gap = ui
            .width()
            .min(76)
            .saturating_sub(7 + display_width(name) + display_width(&tail));
        if gap >= 2 {
            ui.line(format!("  {dot} {name}{}{}", " ".repeat(gap), ui.dim(tail)));
        } else {
            ui.line(format!("  {dot} {name}  {}", ui.dim(tail)));
        }
    }
    ui.blank();
    ui.hint("meow branch <name> to start or switch");
    Ok(0)
}

fn switch_or_create(ui: &Ui, repo: &Repo, requested: &str) -> Result<i32> {
    let name = slugify(requested);
    if name.is_empty() {
        return Err(anyhow::anyhow!(
            "A branch name needs at least one letter or number."
        ));
    }
    if name != requested {
        ui.line(format!("Using `{name}` as the branch name."));
    }
    let valid = repo
        .git
        .probe(&["check-ref-format", "--branch", &name])
        .is_some();
    if !valid {
        return Err(anyhow::anyhow!(
            "`{name}` isn't a usable branch name. Try something like `add-gallery`."
        ));
    }

    let branches = repo.branches()?;
    let exists = branches.iter().any(|b| b.name == name);
    let current = repo.branch().unwrap_or_default();
    if exists && current == name {
        ui.line(format!("You're already on {name}."));
        return Ok(0);
    }

    if !exists {
        if let Some(close) = branches
            .iter()
            .find(|b| crate::util::levenshtein(&b.name, &name) <= 2 && b.name != name)
        {
            if ui.can_prompt() {
                let use_existing = ui.confirm(
                    &format!("`{}` already exists. Did you mean that?", close.name),
                    true,
                )?;
                if use_existing {
                    return switch_to(ui, repo, &close.name, false, &current);
                }
            }
        }
    }

    switch_to(ui, repo, &name, !exists, &current)
}

fn switch_to(ui: &Ui, repo: &Repo, name: &str, create: bool, from: &str) -> Result<i32> {
    let status = repo.status()?;
    let snapshot = journal::create_snapshot(repo, &format!("Before switching to {name}"))?;

    if status.dirty() && ui.can_prompt() {
        ui.blank();
        ui.line("You have unsaved changes.");
        ui.blank();
        let choice = ui.select(
            "What should happen to them?",
            &[
                "Save them here first (recommended)".to_string(),
                "Bring them with me".to_string(),
                "Cancel".to_string(),
            ],
            0,
        )?;
        match choice {
            0 => {
                let code = crate::commands::save::run(ui, None, &[], false, false)?;
                if code != 0 {
                    return Ok(code);
                }
            }
            2 => {
                ui.line("Nothing changed.");
                return Ok(0);
            }
            _ => {}
        }
    }

    let out = if create {
        repo.git.run_main(&["switch", "-c", name])?
    } else {
        repo.git.run_main(&["switch", name])?
    };
    if !out.success() {
        ui.failure(format!("Meow couldn't switch to {name}."));
        for line in out.stderr.lines().take(10) {
            ui.line(ui.dim(line));
        }
        ui.blank();
        ui.line("Your changes are safe. Run `meow changes` to see them.");
        return Ok(1);
    }

    if !from.is_empty() {
        let mut journal = Journal::load(&repo.git_dir);
        let mut action = Action::new(
            "branch",
            if create {
                format!("Started branch {name}")
            } else {
                format!("Switched to {name}")
            },
        );
        action.snapshot = Some(snapshot);
        action.data = json!({ "from": from, "to": name, "created": create });
        let _ = journal.record(&repo.git_dir, action);
    }

    if ui.plain {
        println!("branch {name} created {create}");
        return Ok(0);
    }
    if create {
        ui.success(format!("Started {name}. You're on it now."));
        ui.next(&[(
            "meow save \"First save on this branch\"",
            "checkpoint your work",
        )]);
    } else {
        ui.success(format!("Switched to {name}."));
    }
    Ok(0)
}

fn delete_branch(ui: &Ui, repo: &Repo, name: Option<&str>, force: bool) -> Result<i32> {
    let Some(name) = name else {
        return Err(anyhow::anyhow!(
            "Which branch? Try `meow branch delete add-gallery`."
        ));
    };
    let current = repo.branch().unwrap_or_default();
    if name == current {
        ui.failure(format!("You're on {name}. Switch to another branch first."));
        ui.next(&[("meow branch main", "switch away from it")]);
        return Ok(1);
    }
    let branches = repo.branches()?;
    let Some(branch) = branches.iter().find(|b| b.name == name) else {
        ui.failure(format!("There's no branch called {name}."));
        return Ok(1);
    };

    if !branch.merged && !force {
        let unique = repo
            .git
            .probe(&["rev-list", "--count", &format!("{current}..{name}")])
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(0);
        if ui.plain {
            return Err(ui.decision_required(
                &format!("{name} has {unique} saves that aren't combined into {current}."),
                &format!("Pass --force to delete it after a safety snapshot, or run `meow merge {name}` first."),
            ));
        }
        if !ui.can_prompt() {
            return Err(ui.decision_required(
                &format!("{name} has {unique} saves that aren't combined into {current}."),
                &format!("Pass --force to delete it (a hidden copy is kept), or run `meow merge {name}` first."),
            ));
        }
        ui.warning(format!(
            "{name} has {unique} {} that aren't combined into {current}.",
            if unique == 1 { "save" } else { "saves" }
        ));
        ui.line("Meow will keep a hidden copy so `meow recover` can bring it back.");
        if !ui.confirm(&format!("Delete {name}?"), false)? {
            ui.line("Nothing was deleted.");
            return Ok(0);
        }
    }

    let snapshot = journal::create_snapshot(repo, &format!("Before deleting branch {name}"))?;
    let deleted_ref = format!("refs/meow/deleted/{name}-{}", journal::new_id());
    repo.git.check(&["update-ref", &deleted_ref, &branch.sha])?;
    repo.git.check(&["branch", "-D", name])?;

    let mut journal = Journal::load(&repo.git_dir);
    let mut action = Action::new("branch-delete", format!("Deleted branch {name}"));
    action.snapshot = Some(snapshot);
    action.data = json!({ "name": name, "sha": branch.sha, "ref": deleted_ref });
    let _ = journal.record(&repo.git_dir, action);

    if ui.plain {
        println!("deleted branch {name}");
        return Ok(0);
    }
    ui.success(format!("Deleted {name}."));
    ui.line(
        ui.dim("Its saves are kept privately for 30 days — `meow recover` can bring them back."),
    );
    Ok(0)
}
