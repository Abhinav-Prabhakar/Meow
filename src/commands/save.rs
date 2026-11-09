//! `meow save` — one checkpoint of everything that changed.
//!
//! There is no staging area. The default is "save everything I can see".
//! `--only` exists as an escape hatch, not as a concept to learn.

use std::fs;
use std::path::Path;

use anyhow::{anyhow, Result};
use regex::Regex;
use serde_json::json;

use crate::git::{classify, Failure};
use crate::journal::{Action, Journal};
use crate::repo::{ChangeKind, FileChange, Repo};
use crate::ui::{Sym, Ui};
use crate::util::{author_label, human_size, relative_time, require_repo, suggest_note};

struct SecretHit {
    path: String,
    reason: String,
}

pub fn run(
    ui: &Ui,
    message: Option<&str>,
    only: &[String],
    allow_secrets: bool,
    allow_large: bool,
) -> Result<i32> {
    let repo = require_repo(ui)?;
    let status = repo.status()?;

    if status.conflicted() {
        ui.failure(format!(
            "{} {}",
            status.conflicts.len(),
            if status.conflicts.len() == 1 {
                "file needs your help"
            } else {
                "files need your help"
            }
        ));
        ui.blank();
        ui.line("You and someone else changed the same parts. Nothing is lost.");
        ui.next(&[
            ("meow resolve", "fix them step by step"),
            ("meow cancel", "stop the combine instead"),
        ]);
        return Ok(1);
    }

    let selected = if only.is_empty() {
        status.files.clone()
    } else {
        filter_only(&status.files, only)
    };

    if selected.is_empty() {
        if only.is_empty() {
            nothing_to_save(ui, &repo)?;
        } else {
            ui.line("Nothing has changed in those paths.");
        }
        return Ok(0);
    }

    // ---- secrets -----------------------------------------------------------
    let hits = scan_secrets(&repo, &selected);
    let mut ignore_paths: Vec<String> = Vec::new();
    if !hits.is_empty() && !allow_secrets {
        ui.warning(format!(
            "{} {}",
            hits.len(),
            if hits.len() == 1 {
                "file looks like it holds a secret"
            } else {
                "files look like they hold secrets"
            }
        ));
        for hit in &hits {
            ui.line(format!("  {}  {}", hit.path, ui.dim(&hit.reason)));
        }
        ui.blank();
        ui.line("Saving these can put passwords or keys in your project's history.");
        if ui.can_prompt() {
            let choice = ui.select(
                "What should happen?",
                &[
                    "Ignore them and save the rest".to_string(),
                    "Save them anyway".to_string(),
                    "Cancel".to_string(),
                ],
                0,
            )?;
            match choice {
                0 => ignore_paths = hits.iter().map(|h| h.path.clone()).collect(),
                1 => {}
                _ => {
                    ui.line("Nothing was saved.");
                    return Ok(0);
                }
            }
        } else {
            return Err(ui.decision_required(
                "Some files look like they hold secrets.",
                "Pass --allow-secrets to save them anyway, or add them to .gitignore.",
            ));
        }
    }

    // ---- large files -------------------------------------------------------
    if !allow_large {
        let threshold = crate::config::Config::load().large_file_mb();
        let mut large = Vec::new();
        for file in &selected {
            if file.kind == ChangeKind::Deleted || file.is_untracked() {
                continue;
            }
            let full = repo.root.join(&file.path);
            if let Ok(meta) = fs::metadata(&full) {
                if meta.len() > threshold * 1_000_000 {
                    large.push((file.path.clone(), meta.len()));
                }
            }
        }
        if !large.is_empty() {
            ui.warning("Some files are very large.");
            for (path, size) in &large {
                ui.line(format!("  {}  {}", path, ui.dim(human_size(*size))));
            }
            ui.blank();
            ui.line("Large files make every future get slow for everyone.");
            if ui.can_prompt() {
                let choice = ui.select(
                    "What should happen?",
                    &[
                        "Ignore them and save the rest".to_string(),
                        "Save them anyway".to_string(),
                        "Cancel".to_string(),
                    ],
                    0,
                )?;
                match choice {
                    0 => {
                        for (path, _) in large {
                            ignore_paths.push(path);
                        }
                    }
                    1 => {}
                    _ => {
                        ui.line("Nothing was saved.");
                        return Ok(0);
                    }
                }
            } else {
                return Err(ui.decision_required(
                    "Some files are very large.",
                    "Pass --allow-large to save them anyway, or add them to .gitignore.",
                ));
            }
        }
    }

    // ---- note --------------------------------------------------------------
    let suggestion = suggest_note(&selected);
    let note = match message {
        Some(m) if !m.trim().is_empty() => m.trim().to_string(),
        Some(_) | None => {
            if ui.can_prompt() && message.is_none() {
                ui.hint("A short phrase in the present tense works well — 'Add project photos'.");
                let entered = ui.input("Note", &suggestion)?;
                if entered.trim().is_empty() {
                    suggestion
                } else {
                    entered.trim().to_string()
                }
            } else {
                suggestion
            }
        }
    };

    // ---- ignore the risky ones --------------------------------------------
    for path in &ignore_paths {
        add_ignore(&repo, path)?;
    }

    if !ignore_paths.is_empty() {
        let status = repo.status()?;
        if status.files.is_empty() {
            ui.blank();
            ui.success("The risky files are now ignored. Nothing else needed saving.");
            return Ok(0);
        }
    }

    // ---- stage and commit --------------------------------------------------
    let head_before = repo.head();
    let count = if only.is_empty() {
        let status = repo.status()?;
        status.files.len().max(1)
    } else {
        selected.len()
    };
    if !ui.plain {
        ui.line(format!(
            "Saving {} {}…",
            count,
            if count == 1 { "change" } else { "changes" }
        ));
        ui.blank();
    }

    if only.is_empty() {
        let add = repo.git.run(&["add", "-A"])?;
        if !add.success() {
            return Err(anyhow!("Couldn't prepare your changes for saving."));
        }
    } else {
        let mut args: Vec<&str> = vec!["add", "--"];
        args.extend(only.iter().map(String::as_str));
        repo.git.check(&args)?;
    }

    let commit = if only.is_empty() {
        repo.git.run_main(&["commit", "-m", &note])?
    } else {
        let mut args: Vec<&str> = vec!["commit", "--only", "-m", &note, "--"];
        args.extend(only.iter().map(String::as_str));
        repo.git.run_main(&args)?
    };

    if !commit.success() {
        show_commit_failure(ui, &commit)?;
        return Ok(1);
    }

    let head_after = repo.head();
    let mut journal = Journal::load(&repo.git_dir);
    let mut action = Action::new("save", note.clone());
    action.head_before = head_before;
    action.head_after = head_after;
    action.data = json!({
        "note": note,
        "files": selected.iter().map(|f| f.path.clone()).collect::<Vec<_>>(),
    });
    let _ = journal.record(&repo.git_dir, action);

    if ui.plain {
        let short = repo
            .head()
            .map(|s| s.chars().take(8).collect::<String>())
            .unwrap_or_default();
        let branch = repo.branch().unwrap_or_else(|| "main".to_string());
        println!(
            "saved {short} {branch} \"{note}\" {}",
            crate::util::count(count, "file")
        );
        return Ok(0);
    }

    let branch = repo.branch().unwrap_or_else(|| "main".to_string());
    let right = format!("{} · just now", branch);
    let left = format!("{} Saved \"{}\"", ui.sym(Sym::Success), note);
    ui.line(format!("{left}{}{}", " ".repeat(2), ui.dim(right)));

    let status = repo.status()?;
    let unsent = crate::commands::home::unsent_count(&repo, &status);
    if unsent > 0 {
        ui.line(format!(
            "{} {} ready to send.",
            unsent,
            if unsent == 1 { "save is" } else { "saves are" }
        ));
        if repo.home_label().is_some() {
            ui.next(&[("meow send", "put them online")]);
        } else {
            ui.next(&[("meow send", "put your project online")]);
        }
    } else if repo.home_label().is_none() {
        ui.next(&[("meow send", "put your project online")]);
    }
    Ok(0)
}

fn nothing_to_save(ui: &Ui, repo: &Repo) -> Result<()> {
    if ui.plain {
        println!("nothing to save");
        return Ok(());
    }
    ui.line("Nothing has changed since your last save.");
    if repo.has_commits() {
        if let Ok(info) = repo.head_info() {
            let me = repo.identity().map(|(n, _)| n);
            ui.blank();
            ui.line(ui.dim(format!(
                "Last save: \"{}\" {} · {}",
                info.subject,
                author_label(&info.author, me.as_deref()),
                relative_time(info.when)
            )));
        }
    }
    Ok(())
}

fn show_commit_failure(ui: &Ui, out: &crate::git::GitOutput) -> Result<()> {
    let err = crate::git::GitError {
        stdout: out.stdout.clone(),
        stderr: out.stderr.clone(),
    };
    let failure = classify(&err);
    if failure == Failure::Identity {
        ui.failure("Meow needs a name to put on your saves.");
        ui.next(&[
            ("meow doctor --fix", "set your name and email"),
            ("meow save", "try again"),
        ]);
        return Ok(());
    }
    ui.failure(failure.message());
    if !out.stderr.trim().is_empty() {
        ui.blank();
        for line in out.stderr.lines().take(24) {
            ui.line(ui.dim(line));
        }
    }
    ui.blank();
    ui.line(failure.hint());
    Ok(())
}

fn filter_only(files: &[FileChange], only: &[String]) -> Vec<FileChange> {
    files
        .iter()
        .filter(|f| {
            only.iter().any(|p| {
                f.path == *p || f.path.starts_with(&format!("{}/", p.trim_end_matches('/')))
            })
        })
        .cloned()
        .collect()
}

fn scan_secrets(repo: &Repo, files: &[FileChange]) -> Vec<SecretHit> {
    let name_patterns = [
        ".env",
        "id_rsa",
        "id_ed25519",
        "credentials.json",
        ".npmrc",
        ".pypirc",
        "secrets.json",
        "secrets.yaml",
        "secrets.toml",
    ];
    let content_patterns = [
        (
            r"-----BEGIN (RSA |OPENSSH |EC |DSA |PGP )?PRIVATE KEY-----",
            "a private key",
        ),
        (r"AKIA[0-9A-Z]{16}", "an AWS access key"),
        (r"gh[pousr]_[A-Za-z0-9]{20,}", "a GitHub token"),
        (r"sk-[A-Za-z0-9]{20,}", "an API token"),
        (r"xox[baprs]-[A-Za-z0-9-]{10,}", "a Slack token"),
        (r"AIza[0-9A-Za-z_-]{30,}", "a Google API key"),
    ];
    let regexes: Vec<(Regex, &str)> = content_patterns
        .iter()
        .filter_map(|(re, label)| Regex::new(re).ok().map(|r| (r, *label)))
        .collect();

    let mut hits = Vec::new();
    for file in files {
        if file.kind == ChangeKind::Deleted {
            continue;
        }
        let lower = file.path.to_lowercase();
        let base = Path::new(&lower)
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        let by_name = name_patterns.iter().any(|p| {
            base == *p
                || (p.starts_with(".env") && base.starts_with(".env"))
                || (p.ends_with(".pem") && base.ends_with(".pem"))
                || (p.ends_with(".key") && base.ends_with(".key"))
        }) || base.ends_with(".pem")
            || base.ends_with(".key");
        if by_name {
            hits.push(SecretHit {
                path: file.path.clone(),
                reason: "looks like a secret file".to_string(),
            });
            continue;
        }
        let full = repo.root.join(&file.path);
        let Ok(meta) = fs::metadata(&full) else {
            continue;
        };
        if meta.len() > 2_000_000 {
            continue;
        }
        let Ok(text) = fs::read_to_string(&full) else {
            continue;
        };
        for (re, label) in &regexes {
            if re.is_match(&text) {
                hits.push(SecretHit {
                    path: file.path.clone(),
                    reason: format!("contains what looks like {label}"),
                });
                break;
            }
        }
    }
    hits
}

/// Add a path to `.gitignore` and stop tracking it if it is tracked.
pub fn add_ignore(repo: &Repo, path: &str) -> Result<()> {
    let ignore = repo.root.join(".gitignore");
    let cleaned = path.trim_end_matches('/');
    let mut body = fs::read_to_string(&ignore).unwrap_or_default();
    let already = body
        .lines()
        .any(|l| l.trim() == cleaned || l.trim() == format!("{cleaned}/"));
    if !already {
        if !body.is_empty() && !body.ends_with('\n') {
            body.push('\n');
        }
        body.push_str(cleaned);
        body.push('\n');
        fs::write(&ignore, body)?;
    }
    let tracked = repo
        .git
        .probe(&["ls-files", "--error-unmatch", "--", cleaned])
        .is_some();
    if tracked {
        let _ = repo
            .git
            .run(&["rm", "--cached", "-r", "--ignore-unmatch", "--", cleaned]);
    }
    Ok(())
}
