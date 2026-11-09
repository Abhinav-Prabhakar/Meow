//! `meow ignore` — leave a file or folder out of your project.

use std::fs;

use anyhow::Result;
use serde_json::json;

use crate::journal::{Action, Journal};
use crate::repo::Repo;
use crate::ui::Ui;
use crate::util::require_repo;

pub fn run(ui: &Ui, path: Option<&str>, list: bool) -> Result<i32> {
    let repo = require_repo(ui)?;
    if list || path.is_none() {
        return show(ui, &repo, list);
    }
    let path = path.unwrap_or_default();
    if !confirm_pattern(ui, path)? {
        return Ok(0);
    }
    apply(ui, &repo, path)
}

fn show(ui: &Ui, repo: &Repo, list_only: bool) -> Result<i32> {
    let ignored = ignored_files(repo);
    let existing = read_gitignore(repo);

    if ui.plain {
        for pattern in &existing {
            println!("ignored {pattern}");
        }
        for file in &ignored {
            println!("ignored-file {file}");
        }
        return Ok(0);
    }

    ui.title("Ignored files");
    ui.blank();
    if existing.is_empty() && ignored.is_empty() {
        ui.line("Nothing is ignored yet.");
    } else {
        for pattern in &existing {
            ui.line(format!("  {}  {}", ui.dim("·"), pattern));
        }
        if !ignored.is_empty() {
            ui.blank();
            ui.line(ui.dim(format!(
                "{} currently left out",
                crate::util::count(ignored.len(), "file")
            )));
            for file in ignored.iter().take(8) {
                ui.line(format!("  {}", ui.dim(file)));
            }
            if ignored.len() > 8 {
                ui.line(ui.dim(format!("  … and {} more", ignored.len() - 8)));
            }
        }
    }
    if list_only {
        return Ok(0);
    }

    ui.blank();
    let suggestions = suggestions(repo, &existing);
    if suggestions.is_empty() {
        ui.hint("meow ignore <path> to leave something out");
        return Ok(0);
    }
    ui.line(ui.bold("Worth ignoring"));
    let labels: Vec<String> = suggestions
        .iter()
        .map(|(path, why)| format!("{path}  {}", ui.dim(why)))
        .collect();
    let defaults = vec![true; labels.len()];
    let picked = ui.multi_select("Add these to .gitignore?", &labels, &defaults)?;
    let chosen: Vec<String> = picked
        .into_iter()
        .filter_map(|i| suggestions.get(i).map(|(p, _)| p.clone()))
        .collect();
    if chosen.is_empty() {
        ui.line("Nothing was added.");
        return Ok(0);
    }
    for pattern in &chosen {
        apply(ui, repo, pattern)?;
    }
    Ok(0)
}

fn suggestions(repo: &Repo, existing: &[String]) -> Vec<(String, &'static str)> {
    let mut out: Vec<(String, &'static str)> = Vec::new();
    let has = |p: &str| {
        existing
            .iter()
            .any(|e| e.trim_end_matches('/') == p.trim_end_matches('/'))
    };
    let mut add = |path: &str, why: &'static str| {
        if !has(path) {
            out.push((path.to_string(), why));
        }
    };
    if repo.root.join("node_modules").is_dir() {
        add("node_modules/", "installed packages");
    }
    if repo.root.join("target").is_dir() {
        add("target/", "build output");
    }
    if repo.root.join(".venv").is_dir() || repo.root.join("venv").is_dir() {
        add(".venv/", "Python environment");
    }
    if repo.root.join("__pycache__").is_dir() {
        add("__pycache__/", "Python cache");
    }
    if repo.root.join("dist").is_dir() {
        add("dist/", "built files");
    }
    if repo.root.join(".DS_Store").exists() {
        add(".DS_Store", "macOS clutter");
    }
    if repo.root.join("Thumbs.db").exists() {
        add("Thumbs.db", "Windows clutter");
    }
    if repo.root.join(".env").exists() {
        add(".env", "local secrets");
    }
    for editor in [".vscode", ".idea"] {
        if repo.root.join(editor).is_dir() {
            out.push((format!("{editor}/"), "editor settings"));
        }
    }
    out
}

fn confirm_pattern(ui: &Ui, path: &str) -> Result<bool> {
    let cleaned = path.trim();
    if cleaned.is_empty() || cleaned == ".git" || cleaned.starts_with(".git/") {
        ui.failure("Meow won't ignore the `.git` folder.");
        ui.line("It holds your project's history. Everything else depends on it.");
        return Ok(false);
    }
    if cleaned == "*" || cleaned == "/" || cleaned == "**" {
        ui.warning(format!(
            "`{cleaned}` would ignore everything in the project."
        ));
        if !ui.can_prompt() {
            return Err(ui.decision_required(
                "That pattern would ignore everything.",
                "Use a more specific path.",
            ));
        }
        return ui.confirm("Ignore everything anyway?", false);
    }
    if cleaned.starts_with("*.") || cleaned.contains('/') {
        let looks_like_source = [
            ".rs", ".js", ".ts", ".tsx", ".jsx", ".py", ".go", ".java", ".c", ".cpp", ".h", ".md",
            ".toml", ".json", ".yaml", ".yml", ".html", ".css",
        ]
        .iter()
        .any(|ext| cleaned.ends_with(ext));
        if looks_like_source {
            ui.warning(format!(
                "`{cleaned}` looks like it would match files you probably want to keep."
            ));
            if !ui.can_prompt() {
                return Err(ui.decision_required(
                    &format!("`{cleaned}` would ignore source files."),
                    "Use a more specific path.",
                ));
            }
            return ui.confirm("Ignore them anyway?", false);
        }
    }
    Ok(true)
}

fn apply(ui: &Ui, repo: &Repo, path: &str) -> Result<i32> {
    let cleaned = path.trim().trim_end_matches('/');
    let tracked = repo
        .git
        .probe(&["ls-files", "--", cleaned])
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false);
    let in_history = if tracked {
        repo.git
            .probe(&["log", "--oneline", "--", cleaned])
            .map(|s| !s.trim().is_empty())
            .unwrap_or(false)
    } else {
        false
    };

    let already = read_gitignore(repo)
        .iter()
        .any(|l| l.trim() == cleaned || l.trim() == format!("{cleaned}/"));
    if !already {
        crate::commands::save::add_ignore(repo, cleaned)?;
    } else if tracked {
        let _ = repo
            .git
            .run(&["rm", "--cached", "-r", "--ignore-unmatch", "--", cleaned]);
    }

    let mut action = Action::new("ignore", format!("Ignored {cleaned}"));
    let tracked_list: Vec<String> = if tracked {
        vec![cleaned.to_string()]
    } else {
        Vec::new()
    };
    action.data = json!({ "patterns": [cleaned], "tracked": tracked_list });
    let mut journal = Journal::load(&repo.git_dir);
    let _ = journal.record(&repo.git_dir, action);

    if ui.plain {
        println!("ignored {cleaned} tracked {tracked}");
        return Ok(0);
    }
    if already && !tracked {
        ui.line(format!("`{cleaned}` is already ignored."));
        return Ok(0);
    }
    ui.success(format!("Ignoring {cleaned}."));
    if tracked {
        ui.line(ui.dim("It stays on your disk — it just won't be part of future saves."));
    }
    if in_history {
        ui.blank();
        ui.warning(format!("{cleaned} is already in your project's history."));
        ui.line("Ignoring it now stops future saves, but older saves still contain it.");
        ui.line("If it holds a password or key, change that password now.");
        ui.line(ui.dim("Meow won't rewrite history on its own."));
    }
    Ok(0)
}

fn read_gitignore(repo: &Repo) -> Vec<String> {
    fs::read_to_string(repo.root.join(".gitignore"))
        .unwrap_or_default()
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(str::to_string)
        .collect()
}

fn ignored_files(repo: &Repo) -> Vec<String> {
    repo.git
        .probe(&[
            "ls-files",
            "--others",
            "--ignored",
            "--exclude-standard",
            "-z",
        ])
        .map(|s| {
            s.split('\0')
                .filter(|p| !p.trim().is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}
