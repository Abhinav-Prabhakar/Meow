//! `meow learn` — a safe, ten-minute practice session.
//!
//! Everything happens in a throwaway folder with its own simulated home, so
//! no real project, account, or network is involved. Quitting is always fine.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};

use crate::ui::{display_width, Sym, Ui};

pub fn run(ui: &Ui) -> Result<i32> {
    if !ui.can_prompt() {
        if ui.plain {
            println!("learn chapters 8 sandbox ~/meow-practice");
            return Ok(0);
        }
        ui.title("Practice");
        ui.blank();
        ui.line("`meow learn` is an interactive ten-minute practice session.");
        ui.line("Run it in a terminal to walk through:");
        for (n, title) in CHAPTERS.iter().enumerate() {
            ui.line(format!("  {}. {}", n + 1, title));
        }
        ui.blank();
        ui.line(ui.dim("Nothing real is touched. A practice folder is created in your home."));
        return Ok(0);
    }

    let sandbox = match prepare(ui)? {
        Some(path) => path,
        None => return Ok(0),
    };

    ui.blank();
    ui.line(format!("  {}  Practice", ui.sym(Sym::Cat)));
    ui.blank();
    ui.line("Everything here is fake except Git itself.");
    ui.line(format!(
        "Your practice folder is {} — delete it any time.",
        ui.cyan(crate::util::display_path(&sandbox))
    ));
    ui.line("You can leave whenever you like. Nothing outside this folder changes.");
    ui.blank();

    // 1 — save
    chapter(
        ui,
        1,
        "Save your work",
        "A save is a checkpoint you can come back to.",
    )?;
    fs::write(sandbox.join("hello.txt"), "my first file\n")?;
    git(&sandbox, &["add", "-A"])?;
    git(
        &sandbox,
        &[
            "-c",
            "user.name=Practice You",
            "-c",
            "user.email=practice@example.com",
            "commit",
            "-q",
            "-m",
            "Add hello.txt",
        ],
    )?;
    ui.success("Saved \"Add hello.txt\".");
    ui.line(ui.dim("Under the hood: git add -A && git commit -m \"…\""));
    ui.blank();
    ui.line("In a real project you'd type:");
    ui.line(format!("  {}", ui.cyan("meow save \"Add hello.txt\"")));
    pause(ui)?;

    // 2 — changes
    chapter(
        ui,
        2,
        "See what changed",
        "Meow shows a diff only when you ask for one.",
    )?;
    fs::write(sandbox.join("hello.txt"), "my first file\none more line\n")?;
    let diff = git(&sandbox, &["diff", "--color=never"])?;
    for line in diff.lines().take(12) {
        ui.line(format!("  {line}"));
    }
    ui.blank();
    ui.line(format!("  {}", ui.cyan("meow changes")));
    ui.line(ui.dim("The `+` line is new; a `-` line would be removed."));
    pause(ui)?;

    // 3 — undo
    chapter(
        ui,
        3,
        "Undo a save",
        "Taking a save back keeps your files unless you say otherwise.",
    )?;
    git(&sandbox, &["add", "-A"])?;
    git(
        &sandbox,
        &[
            "-c",
            "user.name=Practice You",
            "-c",
            "user.email=practice@example.com",
            "commit",
            "-q",
            "-m",
            "Add a second line",
        ],
    )?;
    ui.success("Made a save.");
    git(&sandbox, &["reset", "--soft", "HEAD~1"])?;
    ui.success("Took it back — the change is still in the file, just unsaved.");
    ui.line(ui.dim("git reset --soft HEAD~1"));
    ui.blank();
    ui.line(format!("  {}", ui.cyan("meow undo")));
    pause(ui)?;
    git(&sandbox, &["add", "-A"])?;
    git(
        &sandbox,
        &[
            "-c",
            "user.name=Practice You",
            "-c",
            "user.email=practice@example.com",
            "commit",
            "-q",
            "-m",
            "Add a second line",
        ],
    )?;

    // 4 — branch
    chapter(
        ui,
        4,
        "Try something on a branch",
        "A branch is a parallel version. main stays untouched.",
    )?;
    git(&sandbox, &["switch", "-c", "practice-branch"])?;
    fs::write(sandbox.join("branch.txt"), "work in progress\n")?;
    git(&sandbox, &["add", "-A"])?;
    git(
        &sandbox,
        &[
            "-c",
            "user.name=Practice You",
            "-c",
            "user.email=practice@example.com",
            "commit",
            "-q",
            "-m",
            "Start a branch idea",
        ],
    )?;
    ui.success("You're on practice-branch with a new save.");
    ui.line(format!("  {}", ui.cyan("meow branch practice-branch")));
    pause(ui)?;

    // 5 — merge
    chapter(
        ui,
        5,
        "Combine it back",
        "Combining brings the saves from one branch into another.",
    )?;
    git(&sandbox, &["switch", "-q", "main"])?;
    ui.line("Back on main. The branch's file isn't here yet.");
    git(
        &sandbox,
        &[
            "-c",
            "user.name=Practice You",
            "-c",
            "user.email=practice@example.com",
            "merge",
            "--no-edit",
            "practice-branch",
        ],
    )?;
    ui.success("Combined. branch.txt is now on main too.");
    ui.line(ui.dim("git merge practice-branch"));
    ui.line(format!("  {}", ui.cyan("meow merge practice-branch")));
    pause(ui)?;

    // 6 — conflict
    chapter(
        ui,
        6,
        "Fix a conflict",
        "A conflict means two versions of the same line. Nothing is lost.",
    )?;
    fs::write(
        sandbox.join("hello.txt"),
        "my first file\nthe main version\n",
    )?;
    git(&sandbox, &["add", "-A"])?;
    git(
        &sandbox,
        &[
            "-c",
            "user.name=Practice You",
            "-c",
            "user.email=practice@example.com",
            "commit",
            "-q",
            "-m",
            "Main says this",
        ],
    )?;
    git(&sandbox, &["switch", "-q", "practice-branch"])?;
    fs::write(
        sandbox.join("hello.txt"),
        "my first file\nthe branch version\n",
    )?;
    git(&sandbox, &["add", "-A"])?;
    git(
        &sandbox,
        &[
            "-c",
            "user.name=Practice You",
            "-c",
            "user.email=practice@example.com",
            "commit",
            "-q",
            "-m",
            "Branch says this",
        ],
    )?;
    git(&sandbox, &["switch", "-q", "main"])?;
    let merge = Command::new("git")
        .args([
            "-c",
            "user.name=Practice You",
            "-c",
            "user.email=practice@example.com",
            "merge",
            "--no-edit",
            "practice-branch",
        ])
        .current_dir(&sandbox)
        .output()?;
    if !merge.status.success() {
        ui.warning("A conflict appeared in hello.txt, just like in real life.");
        let text = fs::read_to_string(sandbox.join("hello.txt")).unwrap_or_default();
        for line in text.lines().take(8) {
            ui.line(format!("  {}", ui.dim(line)));
        }
        ui.blank();
        ui.line("`meow resolve` would show both sides and ask, part by part.");
        ui.line("For practice, let's keep the main version and finish:");
        let _ = Command::new("git")
            .args(["checkout", "--ours", "--", "hello.txt"])
            .current_dir(&sandbox)
            .output();
        git(&sandbox, &["add", "hello.txt"])?;
        git(
            &sandbox,
            &[
                "-c",
                "user.name=Practice You",
                "-c",
                "user.email=practice@example.com",
                "commit",
                "-q",
                "--no-edit",
            ],
        )?;
        ui.success("Resolved and combined.");
        ui.line(format!("  {}", ui.cyan("meow resolve")));
    }
    pause(ui)?;

    // 7 — share and get
    chapter(
        ui,
        7,
        "Share and get",
        "A home is a copy that lives online. Meow uses a simulated one here.",
    )?;
    let base = std::env::temp_dir().join("meow-practice");
    fs::create_dir_all(&base)?;
    let home = base.join("home.git");
    if !home.exists() {
        Command::new("git")
            .args(["init", "-q", "--bare", "home.git"])
            .current_dir(&base)
            .output()?;
    }
    let has_origin = Command::new("git")
        .args(["remote"])
        .current_dir(&sandbox)
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).contains("origin"))
        .unwrap_or(false);
    if !has_origin {
        git(
            &sandbox,
            &["remote", "add", "origin", home.to_string_lossy().as_ref()],
        )?;
    }
    git(&sandbox, &["push", "-q", "-u", "origin", "main"])?;
    ui.success("Sent your saves to a practice home (no network used).");
    let clone = base.join("friend");
    if clone.exists() {
        let _ = fs::remove_dir_all(&clone);
    }
    let clone_out = Command::new("git")
        .args([
            "clone",
            "-q",
            home.to_string_lossy().as_ref(),
            clone.to_string_lossy().as_ref(),
        ])
        .output()?;
    if clone_out.status.success() {
        ui.success("Copied it into a second folder, as if a friend did `meow get`.");
    }
    ui.line(format!("  {}", ui.cyan("meow send")));
    ui.line(format!("  {}", ui.cyan("meow get <link>")));
    pause(ui)?;

    // 8 — recover
    chapter(
        ui,
        8,
        "Get something back",
        "Meow keeps safety copies, so most mistakes are reversible.",
    )?;
    let hello = sandbox.join("hello.txt");
    let original = fs::read_to_string(&hello).unwrap_or_default();
    fs::write(&hello, "oops\n")?;
    git(
        &sandbox,
        &[
            "restore",
            "--source",
            "HEAD",
            "--worktree",
            "--",
            "hello.txt",
        ],
    )?;
    if fs::read_to_string(&hello).unwrap_or_default() == original {
        ui.success("Put hello.txt back exactly as it was.");
    }
    ui.line(ui.dim("git restore --source HEAD --worktree -- hello.txt"));
    ui.blank();
    ui.line("Meow also keeps discarded files in a trash for 30 days:");
    ui.line(format!("  {}", ui.cyan("meow recover")));
    ui.blank();
    ui.title("You know the whole tool.");
    ui.blank();
    ui.line("Save, send, get, changes, history, undo — that's the daily loop.");
    ui.line("Branches, combining, conflicts, and recover are there when you need them.");
    ui.line(format!("  {}", ui.cyan("meow help")));
    ui.blank();
    ui.line(ui.dim(format!(
        "Practice folder: {}",
        crate::util::display_path(&sandbox)
    )));
    ui.line(ui.dim("You can keep exploring with the meow commands inside it."));
    Ok(0)
}

const CHAPTERS: [&str; 8] = [
    "Save your work",
    "See what changed",
    "Undo a save",
    "Try something on a branch",
    "Combine it back",
    "Fix a conflict",
    "Share and get",
    "Get something back",
];

fn prepare(ui: &Ui) -> Result<Option<PathBuf>> {
    let home = dirs::home_dir().unwrap_or_else(std::env::temp_dir);
    let sandbox = home.join("meow-practice");
    if sandbox.exists() {
        let non_empty = fs::read_dir(&sandbox)
            .map(|mut d| d.next().is_some())
            .unwrap_or(true);
        if non_empty {
            ui.blank();
            ui.line(format!(
                "There's already a practice folder at {}.",
                crate::util::display_path(&sandbox)
            ));
            let choice = ui.select(
                "What should happen?",
                &[
                    "Start fresh (remove the old practice folder)".to_string(),
                    "Quit".to_string(),
                ],
                0,
            )?;
            if choice == 1 {
                ui.line("Nothing was changed.");
                return Ok(None);
            }
            fs::remove_dir_all(&sandbox)
                .with_context(|| format!("Couldn't clear {}", sandbox.display()))?;
        }
    }
    fs::create_dir_all(&sandbox)?;
    Command::new("git")
        .args(["init", "-q", "-b", "main"])
        .current_dir(&sandbox)
        .status()?;
    Command::new("git")
        .args(["config", "user.name", "Practice You"])
        .current_dir(&sandbox)
        .status()?;
    Command::new("git")
        .args(["config", "user.email", "practice@example.com"])
        .current_dir(&sandbox)
        .status()?;
    Ok(Some(sandbox))
}

fn chapter(ui: &Ui, number: usize, title: &str, why: &str) -> Result<()> {
    ui.blank();
    let label = format!("{number} · {title}");
    ui.line(ui.bold(&label));
    ui.line(ui.paint(
        "─".repeat(display_width(&label).min(60)),
        crate::ui::Paint::Dim,
    ));
    ui.line(why);
    ui.blank();
    Ok(())
}

fn pause(ui: &Ui) -> Result<()> {
    ui.blank();
    ui.pause("Press Enter for the next chapter")?;
    Ok(())
}

fn git(dir: &Path, args: &[&str]) -> Result<String> {
    let out = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .with_context(|| format!("Couldn't run git {}", args.join(" ")))?;
    if !out.status.success() {
        return Err(anyhow::anyhow!(
            "The practice session hit a snag: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}
