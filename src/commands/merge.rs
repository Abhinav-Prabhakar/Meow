//! `meow merge` — combine another branch into this one, with a preview.
//!
//! The screen says "combine" because that is what it means. Git's word is
//! available in `meow help --git`, never required to finish the task.

use anyhow::Result;
use chrono::{DateTime, FixedOffset};
use serde_json::json;

use crate::journal::{self, Action, Journal};
use crate::repo::{DiffStat, Repo};
use crate::ui::{Sym, Ui};
use crate::util::{author_label, relative_time, require_repo, truncate_chars};

pub fn run(ui: &Ui, name: &str, yes: bool) -> Result<i32> {
    let repo = require_repo(ui)?;
    let status = repo.status()?;

    if let Some(op) = status.operation {
        ui.failure(format!(
            "This project is already in the middle of a {}.",
            op.label()
        ));
        ui.next(&[
            ("meow resolve", "finish it"),
            ("meow cancel", "stop it and go back"),
        ]);
        return Ok(1);
    }
    if status.detached {
        ui.failure("You're looking at an old save, not a branch.");
        ui.next(&[("meow branch main", "go back to a branch")]);
        return Ok(1);
    }

    let current = status.branch.clone().unwrap_or_else(|| "main".to_string());
    let target = name.trim();
    let branches = repo.branches()?;
    let Some(other) = branches.iter().find(|b| b.name == target) else {
        ui.failure(format!("There's no branch called {target}."));
        let close = branches
            .iter()
            .map(|b| (crate::util::levenshtein(&b.name, target), &b.name))
            .filter(|(d, _)| *d <= 3)
            .min_by_key(|(d, _)| *d)
            .map(|(_, name)| name.clone());
        match close {
            Some(suggestion) => ui.next(&[(
                &format!("meow merge {suggestion}"),
                "did you mean this one?",
            )]),
            None => ui.next(&[("meow branch", "see your branches")]),
        }
        return Ok(1);
    };
    if other.name == current {
        ui.line(format!("You're already on {current}."));
        return Ok(0);
    }
    if repo.is_ancestor(&other.sha, "HEAD") {
        ui.line(format!(
            "{} Everything on {current} is already here — there's nothing to combine.",
            ui.sym(Sym::Success)
        ));
        return Ok(0);
    }

    // ---- clean tree -------------------------------------------------------
    if status.dirty() {
        if ui.plain {
            return Err(ui.decision_required(
                "There are unsaved changes.",
                "Save or discard them first, then run `meow merge`.",
            ));
        }
        ui.warning("You have unsaved changes.");
        ui.line("Combining works best from a clean checkpoint.");
        ui.blank();
        let choice = ui.select(
            "What should happen to them?",
            &[
                "Save them first (recommended)".to_string(),
                "Cancel".to_string(),
            ],
            0,
        )?;
        if choice == 1 {
            ui.line("Nothing changed.");
            return Ok(0);
        }
        let code = crate::commands::save::run(ui, None, &[], false, false)?;
        if code != 0 {
            return Ok(code);
        }
    }

    // ---- preview ----------------------------------------------------------
    let behind = repo.commits_between("HEAD", &other.sha).len();
    let stat = merge_stat(&repo, &other.sha);
    let fast_forward = repo.is_ancestor("HEAD", &other.sha);
    let conflicts = predict_conflicts(&repo, &other.sha, &current);

    if ui.plain && !yes {
        println!(
            "merge {current} <- {} {behind} {} {} +{} -{}",
            other.name,
            if behind == 1 { "save" } else { "saves" },
            crate::util::count(stat.files, "file"),
            stat.added,
            stat.deleted
        );
        return Err(ui.decision_required(
            &format!("Combine {} into {current}?", other.name),
            "Pass --yes to combine without the preview.",
        ));
    }
    if !ui.plain {
        ui.title(&format!("Combine {} into {}", other.name, current));
        ui.blank();
        ui.line(format!(
            "{} · {} changed · +{} -{}",
            crate::util::count(behind, "save"),
            crate::util::count(stat.files, "file"),
            stat.added,
            stat.deleted
        ));
        if let Some((subject, author, when)) = last_save(&repo, &other.sha) {
            let me = repo.identity().map(|(n, _)| n);
            ui.line(format!(
                "Last save: \"{}\" — {} · {}",
                truncate_chars(&subject, 48),
                author_label(&author, me.as_deref()),
                relative_time(when)
            ));
        }
        ui.blank();
        if fast_forward {
            ui.line(format!(
                "{} hasn't changed since you branched — this simply moves it forward.",
                current
            ));
        } else if conflicts.is_empty() {
            ui.line("Your work and theirs will be combined into one new save.");
        } else {
            ui.warning(format!(
                "This will need your help in {}.",
                crate::util::count(conflicts.len(), "file")
            ));
        }
        ui.blank();
    }

    if !yes && ui.can_prompt() {
        let choice = ui.select(
            "What next?",
            &[
                format!("Combine {} into {}", other.name, current),
                "See exactly what changed".to_string(),
                "Cancel".to_string(),
            ],
            0,
        )?;
        match choice {
            0 => {}
            1 => {
                let out = repo.git.check(&[
                    "diff",
                    if ui.color {
                        "--color=always"
                    } else {
                        "--color=never"
                    },
                    &format!("HEAD...{}", other.sha),
                ])?;
                ui.page(&out.stdout);
                ui.blank();
                if !ui.confirm(&format!("Combine {} into {}?", other.name, current), true)? {
                    ui.line("Nothing changed.");
                    return Ok(0);
                }
            }
            _ => {
                ui.line("Nothing changed.");
                return Ok(0);
            }
        }
    } else if !yes && !ui.plain {
        return Err(ui.decision_required(
            &format!("Combine {} into {current}?", other.name),
            "Pass --yes to combine.",
        ));
    }

    // ---- combine ----------------------------------------------------------
    let snapshot = journal::create_snapshot(&repo, "Before combining branches")?;
    let head_before = repo.head();
    if !ui.plain {
        ui.line(format!("Combining {}…", crate::util::count(behind, "save")));
        ui.blank();
    }
    let target_arg = other.name.as_str();
    let args = if fast_forward {
        vec!["merge", "--ff-only", "--no-edit", target_arg]
    } else {
        vec!["merge", "--no-edit", target_arg]
    };
    let out = repo.git.run_main(&args)?;
    let head_after = repo.head();

    if !out.success() {
        let after = repo.status()?;
        if after.conflicted() || after.operation.is_some() {
            // Record the combine as the user's action, so that once the
            // conflicts are resolved, `meow undo` can take the whole combine
            // back instead of stopping at an internal resolution step.
            let mut action =
                Action::new("merge", format!("Combined {} into {current}", other.name));
            action.head_before = head_before.clone();
            action.snapshot = Some(snapshot);
            action.data = json!({
                "from": other.name,
                "into": current,
                "saves": behind,
                "linear": fast_forward,
                "conflicted": true,
            });
            let mut journal = Journal::load(&repo.git_dir);
            let _ = journal.record(&repo.git_dir, action);

            if ui.plain {
                println!(
                    "conflicts {} {}",
                    after.conflicts.len(),
                    after.conflicts.join(" ")
                );
                return Ok(1);
            }
            ui.failure(format!(
                "{} {} your help",
                crate::util::count(after.conflicts.len(), "file"),
                if after.conflicts.len() == 1 {
                    "needs"
                } else {
                    "need"
                }
            ));
            ui.blank();
            ui.line("You and someone else changed the same parts. Nothing is lost.");
            ui.next(&[
                ("meow resolve", "fix them one at a time"),
                ("meow cancel", "stop and go back"),
            ]);
            return Ok(1);
        }
        ui.failure("The combine didn't finish.");
        for line in out.stderr.lines().take(16) {
            ui.line(ui.dim(line));
        }
        ui.blank();
        ui.line("Nothing was lost. `meow cancel` returns to where you started.");
        return Ok(1);
    }

    let mut journal = Journal::load(&repo.git_dir);
    let mut action = Action::new("merge", format!("Combined {} into {current}", other.name));
    action.head_before = head_before;
    action.head_after = head_after;
    action.snapshot = Some(snapshot);
    action.data = json!({
        "from": other.name,
        "into": current,
        "saves": behind,
        "linear": fast_forward,
    });
    let _ = journal.record(&repo.git_dir, action);

    if ui.plain {
        println!(
            "merged {} into {current} {behind} {}",
            other.name,
            if behind == 1 { "save" } else { "saves" }
        );
        return Ok(0);
    }

    ui.success(format!(
        "Combined. {} now has {}.",
        current,
        crate::util::count(behind, "new save")
    ));
    ui.blank();
    ui.line(ui.dim(format!(
        "{} is kept until you delete it — nothing was lost.",
        other.name
    )));

    if ui.can_prompt() && ui.confirm(&format!("Delete {} now?", other.name), false)? {
        let _ = repo.git.run(&["branch", "-d", &other.name]);
        ui.success(format!("Deleted {}.", other.name));
    }

    let status = repo.status()?;
    let unsent = crate::commands::home::unsent_count(&repo, &status);
    if unsent > 0 && repo.home_label().is_some() {
        ui.next(&[("meow send", "put the combined work online")]);
    }
    Ok(0)
}

fn merge_stat(repo: &Repo, sha: &str) -> DiffStat {
    let mut stat = DiffStat::default();
    let Some(raw) = repo
        .git
        .probe(&["diff", "--numstat", "-z", &format!("HEAD...{sha}")])
    else {
        return stat;
    };
    for rec in raw.split('\0').filter(|s| !s.is_empty()) {
        let mut f = rec.split('\t');
        let added = f.next().unwrap_or("0");
        let deleted = f.next().unwrap_or("0");
        let _path = f.next().unwrap_or("");
        stat.files += 1;
        stat.added += added.parse::<usize>().unwrap_or(0);
        stat.deleted += deleted.parse::<usize>().unwrap_or(0);
    }
    stat
}

fn last_save(repo: &Repo, sha: &str) -> Option<(String, String, DateTime<FixedOffset>)> {
    let raw = repo
        .git
        .probe(&["log", "-1", "--pretty=format:%s%x00%an%x00%aI", sha])?;
    let mut f = raw.split('\0');
    let subject = f.next().unwrap_or_default().to_string();
    let author = f.next().unwrap_or_default().to_string();
    let when = f
        .next()
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())?;
    Some((subject, author, when))
}

/// Ask Git which paths would conflict, without touching the working tree.
fn predict_conflicts(repo: &Repo, sha: &str, current: &str) -> Vec<String> {
    let Ok(out) = repo
        .git
        .run(&["merge-tree", "--write-tree", "--name-only", current, sha])
    else {
        return Vec::new();
    };
    if out.success() {
        return Vec::new();
    }
    // `<tree oid>`, then one path per line, then a blank line, then messages.
    let mut paths = Vec::new();
    for line in out.stdout.lines().skip(1) {
        if line.trim().is_empty() {
            break;
        }
        paths.push(line.trim().to_string());
    }
    paths
}
