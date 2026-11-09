//! `meow help` — plain-language help, examples, and the Git behind it.

use anyhow::Result;

use crate::ui::{display_width, Sym, Ui};

struct Entry {
    name: &'static str,
    summary: &'static str,
    examples: &'static [&'static str],
    git: &'static str,
    mistakes: &'static [&'static str],
}

const ENTRIES: &[Entry] = &[
    Entry {
        name: "meow",
        summary: "Where am I, what is my work doing, and what should I do next?",
        examples: &["meow", "meow --plain"],
        git: "git status",
        mistakes: &[
            "Nothing to fix here — this screen is safe to run any time.",
            "If it says the folder isn't a project, use `meow new` or `meow get <link>`.",
        ],
    },
    Entry {
        name: "save",
        summary: "Save a checkpoint of everything that changed.",
        examples: &[
            "meow save",
            "meow save \"Add the gallery layout\"",
            "meow save --only src/app.js",
        ],
        git: "git add -A && git commit -m \"…\"",
        mistakes: &[
            "A save is local. Use `meow send` to put it online.",
            "If a save is blocked by a secret or a huge file, ignore it or pass the matching --allow flag.",
        ],
    },
    Entry {
        name: "send",
        summary: "Put your saves online, creating a repository if there isn't one.",
        examples: &["meow send"],
        git: "git push (sets the upstream on the first send)",
        mistakes: &[
            "If someone else sent first, Meow gets their work and then sends yours.",
            "A sent save can't be erased from history; `meow undo` offers a reversing save instead.",
        ],
    },
    Entry {
        name: "get",
        summary: "Get the latest saves, or copy a project onto this machine.",
        examples: &["meow get", "meow get github.com/ana/portfolio", "meow get <link> --folder site"],
        git: "git fetch && git merge, or git clone",
        mistakes: &[
            "Unsaved work is set aside, never thrown away.",
            "If files conflict, `meow resolve` walks through them one at a time.",
        ],
    },
    Entry {
        name: "changes",
        summary: "See exactly what changed since the last save.",
        examples: &[
            "meow changes",
            "meow changes src/",
            "meow changes --stat",
            "meow changes --saved 2",
        ],
        git: "git diff HEAD (and a no-index diff for new files)",
        mistakes: &[
            "A binary file shows a summary, not text.",
            "Use `--saved <n>` to look at an older save instead of unsaved work.",
        ],
    },
    Entry {
        name: "history",
        summary: "See your past saves, grouped by day.",
        examples: &[
            "meow history",
            "meow history 3",
            "meow history --all",
            "meow history --file src/app.js",
            "meow history 3 --restore",
        ],
        git: "git log, git show, git restore --source",
        mistakes: &[
            "`--all` includes other branches; without it you see the branch you're on.",
            "Bringing an old version back creates unsaved changes; it doesn't rewrite history.",
        ],
    },
    Entry {
        name: "undo",
        summary: "Take back your last action.",
        examples: &["meow undo", "meow undo 2", "meow undo --list"],
        git: "git reset --soft HEAD~1, or git revert for a sent save",
        mistakes: &[
            "Sent saves are never rewritten. Meow offers a reversing save instead.",
            "Undo works from Meow's own journal, so actions done outside Meow aren't in the list.",
        ],
    },
    Entry {
        name: "redo",
        summary: "Put back what you just undid.",
        examples: &["meow redo"],
        git: "re-applies the recorded action",
        mistakes: &[
            "Redo only knows Meow's last undone action.",
            "If the project changed since then, Meow refuses rather than guessing.",
        ],
    },
    Entry {
        name: "branch",
        summary: "List, start, or switch a parallel version of your project.",
        examples: &[
            "meow branch",
            "meow branch add-gallery",
            "meow branch delete add-gallery",
        ],
        git: "git branch, git switch -c, git switch, git branch -d/-D",
        mistakes: &[
            "Switching with unsaved work asks what to do first.",
            "Deleting an uncombined branch needs explicit confirmation and keeps a hidden copy.",
        ],
    },
    Entry {
        name: "merge",
        summary: "Combine another branch into the one you're on.",
        examples: &["meow merge add-gallery", "meow merge add-gallery --yes"],
        git: "git merge (fast-forward when possible)",
        mistakes: &[
            "Save or discard unsaved changes first.",
            "Conflicts are not an error — `meow resolve` walks through them.",
        ],
    },
    Entry {
        name: "resolve",
        summary: "Fix conflicts one part at a time, in plain words.",
        examples: &[
            "meow resolve",
            "meow resolve --list",
            "meow resolve --ours",
            "meow resolve --theirs",
            "meow resolve --both",
        ],
        git: "git checkout --ours/--theirs, git add, git commit --no-edit",
        mistakes: &[
            "`--ours` means your current branch, `--theirs` means the other one.",
            "Choosing `--both` keeps both versions and marks the join; it doesn't promise valid code.",
        ],
    },
    Entry {
        name: "cancel",
        summary: "Stop a combine and return to exactly where you were.",
        examples: &["meow cancel"],
        git: "git merge --abort",
        mistakes: &[
            "Nothing is deleted. Both versions are kept.",
            "If the project is mid-rebase, Meow names the Git command instead of guessing.",
        ],
    },
    Entry {
        name: "discard",
        summary: "Throw away unsaved changes, keeping a way back.",
        examples: &["meow discard", "meow discard src/experiment.js", "meow discard --all"],
        git: "git restore, with untracked files moved to Meow's trash",
        mistakes: &[
            "Untracked files are moved, not deleted.",
            "`meow recover` can bring discarded work back for 30 days.",
        ],
    },
    Entry {
        name: "ignore",
        summary: "Leave a file or folder out of your project.",
        examples: &[
            "meow ignore node_modules",
            "meow ignore .env",
            "meow ignore --list",
        ],
        git: "edits .gitignore, then git rm --cached -r for tracked paths",
        mistakes: &[
            "Ignoring stops future saves but doesn't remove a file already in history.",
            "If a secret is already in history, change the secret, not just the ignore rule.",
        ],
    },
    Entry {
        name: "recover",
        summary: "Find work you thought you lost.",
        examples: &["meow recover", "meow recover --purge"],
        git: "restores from Meow's private snapshots and trash",
        mistakes: &[
            "Current files are snapshotted before a restore, so recovery is reversible.",
            "`--purge` is permanent and asks first.",
        ],
    },
    Entry {
        name: "doctor",
        summary: "Check that everything is set up.",
        examples: &["meow doctor", "meow doctor --fix", "meow doctor --report"],
        git: "reads config, remotes, and repository health",
        mistakes: &[
            "Doctor never uploads anything by itself.",
            "`--report` writes a redacted file you can choose to share.",
        ],
    },
    Entry {
        name: "learn",
        summary: "Practice in a safe sandbox, with a simulated online home.",
        examples: &["meow learn"],
        git: "runs real Git in a throwaway folder",
        mistakes: &[
            "No real project, account, or network is touched.",
            "Quitting is always allowed; the practice folder can be deleted any time.",
        ],
    },
    Entry {
        name: "version",
        summary: "Show the Meow and Git versions.",
        examples: &["meow version", "meow version --verbose"],
        git: "git --version",
        mistakes: &["Upgrade through the package manager you installed Meow with."],
    },
];

pub fn run(ui: &Ui, command: Option<&str>, git: bool) -> Result<i32> {
    if git {
        return mapping(ui, command);
    }
    match command {
        Some(command) => detail(ui, command),
        None => overview(ui),
    }
}

fn overview(ui: &Ui) -> Result<i32> {
    if ui.plain {
        for entry in ENTRIES {
            println!("{} {}", entry.name, entry.summary);
        }
        return Ok(0);
    }
    ui.blank();
    ui.line(format!("{}  meow", ui.sym(Sym::Cat)));
    ui.blank();
    ui.line(ui.bold("  save, share, and protect your work"));
    ui.blank();

    let everyday = [
        ("meow", "where am I, what's next"),
        ("meow save \"note\"", "save a checkpoint"),
        ("meow send", "put your saves online"),
        ("meow get", "get the latest (or get a copy)"),
        ("meow changes", "see what changed"),
        ("meow history", "see past saves"),
        ("meow undo", "take back your last action"),
    ];
    let occasional = [
        ("meow new", "start a project"),
        ("meow branch", "work on a separate version"),
        ("meow merge", "combine two versions"),
        ("meow resolve", "fix a conflict"),
        ("meow discard", "throw away unsaved changes"),
        ("meow ignore", "stop tracking a file"),
        ("meow recover", "find work you thought you lost"),
        ("meow doctor", "check your setup"),
        ("meow learn", "practice safely"),
    ];
    section(ui, "Everyday", &everyday);
    ui.blank();
    section(ui, "When you need them", &occasional);
    ui.blank();
    ui.line(ui.dim("  meow help <command>     details and examples"));
    ui.line(ui.dim("  meow help --git         what this is called in Git"));
    ui.blank();
    Ok(0)
}

fn section(ui: &Ui, title: &str, rows: &[(&str, &str)]) {
    ui.line(ui.bold(title));
    let width = rows
        .iter()
        .map(|(command, _)| display_width(command))
        .max()
        .unwrap_or(0)
        + 4;
    for (command, hint) in rows {
        let pad = width.saturating_sub(display_width(command));
        ui.line(format!(
            "  {}{}{}",
            ui.cyan(command),
            " ".repeat(pad),
            ui.dim(hint)
        ));
    }
}

fn detail(ui: &Ui, command: &str) -> Result<i32> {
    let wanted = command.trim().trim_start_matches("meow ").trim();
    let Some(entry) = ENTRIES.iter().find(|e| e.name == wanted) else {
        ui.failure(format!("There's no command called {wanted}."));
        let close = ENTRIES
            .iter()
            .map(|e| (crate::util::levenshtein(e.name, wanted), e.name))
            .filter(|(d, _)| *d <= 3)
            .min_by_key(|(d, _)| *d)
            .map(|(_, name)| name);
        match close {
            Some(name) => ui.next(&[(&format!("meow help {name}"), "did you mean this one?")]),
            None => ui.next(&[("meow help", "see all commands")]),
        }
        return Ok(1);
    };

    if ui.plain {
        println!("meow {}", entry.name);
        println!("{}", entry.summary);
        for example in entry.examples {
            println!("example: {example}");
        }
        println!("git: {}", entry.git);
        for mistake in entry.mistakes {
            println!("note: {mistake}");
        }
        return Ok(0);
    }

    ui.title(&format!("meow {}", entry.name));
    ui.blank();
    ui.line(entry.summary);
    ui.blank();
    ui.line(ui.bold("Examples"));
    for example in entry.examples {
        ui.line(format!("  {}", ui.cyan(example)));
    }
    ui.blank();
    ui.line(ui.bold("Under the hood"));
    ui.line(format!("  {}", ui.dim(entry.git)));
    ui.blank();
    ui.line(ui.bold("Common mistakes"));
    for mistake in entry.mistakes {
        ui.line(format!("  {} {}", ui.dim("·"), mistake));
    }
    ui.blank();
    ui.hint("meow help --git to see the whole translation table");
    Ok(0)
}

fn mapping(ui: &Ui, command: Option<&str>) -> Result<i32> {
    let rows: Vec<(&str, &str)> = if let Some(command) = command {
        let wanted = command.trim().trim_start_matches("meow ").trim();
        ENTRIES
            .iter()
            .filter(|e| e.name == wanted)
            .map(|e| (e.name, e.git))
            .collect()
    } else {
        ENTRIES.iter().map(|e| (e.name, e.git)).collect()
    };
    if rows.is_empty() {
        ui.failure(format!(
            "There's no command called {}.",
            command.unwrap_or_default()
        ));
        return Ok(1);
    }
    if ui.plain {
        for (name, git) in rows {
            println!("meow {name} = {git}");
        }
        return Ok(0);
    }
    ui.title("Meow and Git");
    ui.blank();
    let width = rows
        .iter()
        .map(|(name, _)| display_width(name) + 7)
        .max()
        .unwrap_or(0);
    for (name, git) in rows {
        let label = format!("meow {name}");
        let pad = width.saturating_sub(display_width(&label));
        ui.line(format!(
            "  {}{}{}",
            ui.cyan(label),
            " ".repeat(pad),
            ui.dim(git)
        ));
    }
    ui.blank();
    ui.hint("Every command also accepts --explain to show the real Git calls.");
    Ok(0)
}
