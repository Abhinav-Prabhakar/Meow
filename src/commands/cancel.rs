//! `meow cancel` — stop a combine and return to exactly where you were.

use anyhow::Result;

use crate::journal::{self, Action, Journal};
use crate::repo::Operation;
use crate::ui::{Sym, Ui};
use crate::util::require_repo;

pub fn run(ui: &Ui) -> Result<i32> {
    let repo = require_repo(ui)?;
    let status = repo.status()?;
    let Some(op) = status.operation else {
        if ui.plain {
            println!("nothing to cancel");
        } else {
            ui.line(format!(
                "{} There's nothing to cancel.",
                ui.sym(Sym::Success)
            ));
        }
        return Ok(0);
    };

    match op {
        Operation::Merge => {
            ui.blank();
            ui.line("Cancel this combine?");
            ui.blank();
            ui.line("Both versions are kept — nothing is deleted.");
            if let Some(branch) = &status.branch {
                ui.line(format!("You'll be back on {branch} exactly as you were."));
            }
            ui.blank();
            if ui.can_prompt() && !ui.confirm("Cancel the combine?", true)? {
                ui.line("The combine is still open.");
                return Ok(0);
            }
            let snapshot = journal::create_snapshot(&repo, "Before cancelling a combine")?;
            let head_before = repo.head();
            let out = repo.git.run_main(&["merge", "--abort"])?;
            if !out.success() {
                ui.failure("The combine couldn't be cancelled cleanly.");
                for line in out.stderr.lines().take(12) {
                    ui.line(ui.dim(line));
                }
                ui.blank();
                ui.line("Your files are still intact. Run `meow doctor` for a check.");
                return Ok(1);
            }
            let mut action = Action::new("cancel", "Cancelled a combine");
            action.head_before = head_before.clone();
            action.head_after = repo.head();
            action.snapshot = Some(snapshot);
            let mut journal = Journal::load(&repo.git_dir);
            let _ = journal.record(&repo.git_dir, action);

            if ui.plain {
                println!("cancelled combine");
                return Ok(0);
            }
            ui.success("Cancelled. You're back where you started.");
            if let Ok(status) = repo.status() {
                if status.dirty() {
                    ui.line(ui.dim("Your unsaved changes are still here."));
                    ui.next(&[("meow changes", "review them")]);
                }
            }
            Ok(0)
        }
        other => {
            let word = other.label();
            ui.failure(format!("This project is in the middle of a Git {word}."));
            ui.blank();
            ui.line(
                "Meow only manages combines (merges). Finishing or cancelling this one needs Git:",
            );
            ui.line(ui.dim(format!("  git {word} --abort")));
            ui.blank();
            ui.line("If you're not sure, run `meow doctor` and share its report.");
            Ok(1)
        }
    }
}
