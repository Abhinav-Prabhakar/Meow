//! `meow new` — start a project, set it up gently, and leave a clean first state.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};

use crate::config::Config;
use crate::git::Git;
use crate::ui::Ui;
use crate::util::display_path;

pub fn run(ui: &Ui, name: Option<&str>) -> Result<i32> {
    let cwd = std::env::current_dir()?;
    let (target, created_folder) = match name {
        Some(n) => {
            let path = Path::new(n);
            let target = if path.is_absolute() {
                path.to_path_buf()
            } else {
                cwd.join(path)
            };
            if !target.exists() {
                fs::create_dir_all(&target)
                    .map_err(|e| anyhow!("Couldn't create {}: {e}", target.display()))?;
                (target, true)
            } else {
                (target, false)
            }
        }
        None => (cwd.clone(), false),
    };

    let git = Git::new(&target, ui);
    if git.probe(&["rev-parse", "--git-dir"]).is_some() {
        if ui.plain {
            println!("project {}", display_path(&target));
        } else {
            ui.header(&display_path(&target));
            ui.line("This project is already set up. Here's where you are.");
        }
        return crate::commands::home::run(ui);
    }

    let was_empty = is_empty_dir(&target);
    let stack = detect_stack(&target);

    // Identity is only requested when it is actually needed, with consent.
    let identity = ensure_identity(ui, &git)?;

    let branch = choose_branch(&git);
    let init = git.check(&["init", "-b", &branch]);
    if let Err(err) = init {
        let text = err.to_string();
        if text.contains("unknown switch") || text.contains("unknown option") {
            // Very old Git: init without -b, then rename.
            git.check(&["init"])?;
            let _ = git.run(&["symbolic-ref", "HEAD", &format!("refs/heads/{branch}")]);
        } else {
            return Err(err);
        }
    }

    if let Some((name, email)) = identity {
        let _ = git.run(&["config", "user.name", &name]);
        let _ = git.run(&["config", "user.email", &email]);
    }

    let gitignore = target.join(".gitignore");
    let added_ignore = if !gitignore.exists() {
        let patterns = stack.patterns();
        let body = format!(
            "# Added by meow for a {} project\n{}\n\n# Keep local secrets out of your project\n.env\n",
            stack.label,
            patterns
                .iter()
                .map(|p| p.to_string())
                .collect::<Vec<_>>()
                .join("\n")
        );
        fs::write(&gitignore, body)?;
        Some(stack)
    } else {
        Some(Stack {
            label: stack.label,
            patterns: Vec::new(),
        })
    };

    let wrote_readme = if was_empty {
        write_starter_readme(&target)?;
        true
    } else {
        false
    };

    if ui.plain {
        println!("project {}", display_path(&target));
        println!("branch {branch}");
        println!("next: meow save \"First save\"");
        return Ok(0);
    }

    ui.header(&display_path(&target));
    ui.success("Project started");
    if let Some(stack) = added_ignore {
        if !stack.patterns.is_empty() {
            let list = stack
                .patterns
                .iter()
                .take(4)
                .map(|p| p.trim_end_matches('/').to_string())
                .collect::<Vec<_>>()
                .join(", ");
            ui.success(format!(
                "Added a .gitignore for a {} project ({list})",
                stack.label
            ));
        } else {
            ui.line(format!(
                "{} Kept your existing .gitignore",
                ui.sym(crate::ui::Sym::Success)
            ));
        }
    }
    if wrote_readme {
        ui.success("Wrote a starter README.md");
    } else if !was_empty {
        ui.line(ui.dim("Your files were already here — nothing was overwritten."));
    }

    let mut next = Vec::new();
    let cd;
    if created_folder {
        cd = format!(
            "cd {}",
            target.file_name().unwrap_or_default().to_string_lossy()
        );
        next.push((cd.as_str(), "go into the project"));
    }
    next.push(("meow save \"First save\"", "save the starter files"));
    next.push(("meow send", "put it online"));
    ui.next(&next);
    Ok(0)
}

fn ensure_identity(ui: &Ui, git: &Git) -> Result<Option<(String, String)>> {
    let name = git.probe(&["config", "user.name"]).unwrap_or_default();
    let email = git.probe(&["config", "user.email"]).unwrap_or_default();
    if !name.trim().is_empty() && !email.trim().is_empty() {
        return Ok(None);
    }
    if !ui.can_prompt() {
        return Err(ui.decision_required(
            "Meow needs a name and email to put on your saves.",
            "Set them with: git config --global user.name \"Your Name\" and git config --global user.email \"you@example.com\"",
        ));
    }
    ui.warning("Meow needs a name to put on your saves.");
    ui.hint("It's attached to every save, like a signature. You can change it later.");
    ui.blank();
    let name = ui.input("Your name", "")?;
    let email = ui.input("Your email", "")?;
    if name.trim().is_empty() || email.trim().is_empty() {
        return Err(anyhow!(
            "A name and email are needed before the first save."
        ));
    }
    Ok(Some((name, email)))
}

fn choose_branch(git: &Git) -> String {
    if let Some(cfg) = Config::load().default_branch() {
        if !cfg.trim().is_empty() {
            return crate::util::slugify(&cfg);
        }
    }
    if let Some(configured) = git.probe(&["config", "init.defaultBranch"]) {
        if !configured.trim().is_empty() {
            return configured.trim().to_string();
        }
    }
    "main".to_string()
}

#[derive(Clone)]
struct Stack {
    label: &'static str,
    patterns: Vec<&'static str>,
}

impl Stack {
    fn patterns(&self) -> &[&'static str] {
        &self.patterns
    }
}

fn detect_stack(dir: &Path) -> Stack {
    if dir.join("Cargo.toml").exists() {
        return Stack {
            label: "Rust",
            patterns: vec!["target/"],
        };
    }
    if dir.join("package.json").exists()
        || dir.join("pnpm-lock.yaml").exists()
        || dir.join("yarn.lock").exists()
    {
        return Stack {
            label: "Node",
            patterns: vec!["node_modules/", "dist/", ".next/", "*.log"],
        };
    }
    if dir.join("pyproject.toml").exists()
        || dir.join("requirements.txt").exists()
        || dir.join("setup.py").exists()
    {
        return Stack {
            label: "Python",
            patterns: vec!["__pycache__/", ".venv/", "*.pyc", "dist/"],
        };
    }
    if dir.join("go.mod").exists() {
        return Stack {
            label: "Go",
            patterns: vec!["bin/", "*.exe"],
        };
    }
    Stack {
        label: "folder",
        patterns: vec![".DS_Store", "Thumbs.db"],
    }
}

fn is_empty_dir(dir: &Path) -> bool {
    let Ok(mut entries) = fs::read_dir(dir) else {
        return false;
    };
    entries.all(|e| {
        e.map(|e| {
            let name = e.file_name();
            name == ".DS_Store" || name == ".git"
        })
        .unwrap_or(false)
    })
}

fn write_starter_readme(dir: &Path) -> Result<()> {
    let path: PathBuf = dir.join("README.md");
    if path.exists() {
        return Ok(());
    }
    let name = dir
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "my project".to_string());
    let body = format!(
        "# {name}\n\nA new project.\n\n## Working on it\n\n```\nmeow                    where am I, what's next\nmeow save \"note\"        save a checkpoint\nmeow send               put your saves online\nmeow get                get the latest\nmeow undo               take back your last action\n```\n\nThat's the whole loop. `meow help` if you forget.\n"
    );
    fs::write(path, body)?;
    Ok(())
}
