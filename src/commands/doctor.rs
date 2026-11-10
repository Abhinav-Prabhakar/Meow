//! `meow doctor` — check that everything is set up, in plain language.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

use anyhow::Result;
use chrono::Utc;

use crate::journal::{self, Journal};
use crate::repo::Repo;
use crate::ui::{Sym, Ui};
use crate::util::{display_path, human_size};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Level {
    Ok,
    Warn,
    Fail,
}

struct Check {
    level: Level,
    label: String,
    detail: String,
}

pub fn run(ui: &Ui, fix: bool, report: bool) -> Result<i32> {
    let checks = gather(ui);
    let failures = checks.iter().filter(|c| c.level == Level::Fail).count();
    let warnings = checks.iter().filter(|c| c.level == Level::Warn).count();

    if report {
        let path = write_report(&checks)?;
        if ui.plain {
            println!("report {}", path.display());
        } else {
            ui.success(format!(
                "Wrote a redacted report to {}",
                display_path(&path)
            ));
            ui.line(ui.dim("It contains no tokens. Share it only if you want help."));
        }
        return Ok(if failures > 0 { 1 } else { 0 });
    }

    if ui.plain {
        for check in &checks {
            let level = match check.level {
                Level::Ok => "ok",
                Level::Warn => "warn",
                Level::Fail => "fail",
            };
            if check.detail.is_empty() {
                println!("{level} {}", check.label);
            } else {
                println!("{level} {} - {}", check.label, check.detail);
            }
        }
        println!("summary {failures} failures {warnings} warnings");
        return Ok(if failures > 0 { 1 } else { 0 });
    }

    ui.title("Checking your setup");
    ui.blank();
    for check in &checks {
        let sym = match check.level {
            Level::Ok => ui.sym(Sym::Success).to_string(),
            Level::Warn => ui.sym(Sym::Warn).to_string(),
            Level::Fail => ui.sym(Sym::Error).to_string(),
        };
        ui.line(format!("  {sym} {}", check.label));
        if !check.detail.is_empty() {
            ui.line(format!("     {}", ui.dim(&check.detail)));
        }
    }
    ui.blank();

    if failures == 0 && warnings == 0 {
        ui.success("Everything looks good.");
        return Ok(0);
    }
    if failures == 0 {
        ui.line(ui.dim("Meow works. The warnings are worth a look when you have a moment."));
        return Ok(0);
    }

    ui.line(format!(
        "{} {} attention.",
        failures,
        if failures == 1 {
            "thing needs"
        } else {
            "things need"
        }
    ));
    let mut next: Vec<(&str, &str)> = Vec::new();
    if checks.iter().any(|c| c.label.starts_with("Name and email")) {
        next.push(("meow doctor --fix", "set your name and email"));
    }
    next.push(("meow doctor --report", "write a file you can share"));
    ui.next(&next);
    if fix {
        return apply_fixes(ui, &checks);
    }
    Ok(1)
}

fn gather(ui: &Ui) -> Vec<Check> {
    let _ = ui;
    let mut checks = Vec::new();

    // Git
    match Command::new("git").arg("--version").output() {
        Ok(out) if out.status.success() => {
            let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
            let version = text.replace("git version ", "");
            let old = version
                .split('.')
                .take(2)
                .filter_map(|p| p.parse::<u32>().ok())
                .collect::<Vec<_>>();
            if old.len() == 2 && (old[0], old[1]) < (2, 30) {
                checks.push(Check {
                    level: Level::Fail,
                    label: format!("Git {version} is too old"),
                    detail: "Meow needs Git 2.30 or newer. Install a newer Git, then try again."
                        .to_string(),
                });
            } else {
                checks.push(Check {
                    level: Level::Ok,
                    label: format!("Git {version} found"),
                    detail: String::new(),
                });
            }
        }
        _ => {
            checks.push(Check {
                level: Level::Fail,
                label: "Git isn't installed (or isn't on your PATH)".to_string(),
                detail: install_hint(),
            });
        }
    }

    // Identity
    let name = git_config("user.name");
    let email = git_config("user.email");
    if name.is_some() && email.is_some() {
        checks.push(Check {
            level: Level::Ok,
            label: format!("Name and email set ({})", name.unwrap_or_default()),
            detail: String::new(),
        });
    } else {
        checks.push(Check {
            level: Level::Fail,
            label: "Name and email aren't set".to_string(),
            detail: "Every save carries a name. Run `meow doctor --fix`.".to_string(),
        });
    }

    // Optional helpers
    if Command::new("gh")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
    {
        let signed_in = Command::new("gh")
            .args(["auth", "status"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        checks.push(Check {
            level: if signed_in { Level::Ok } else { Level::Warn },
            label: if signed_in {
                "GitHub CLI is signed in".to_string()
            } else {
                "GitHub CLI is installed but not signed in".to_string()
            },
            detail: if signed_in {
                String::new()
            } else {
                "`meow send` will ask you to sign in, or you can paste a repository link."
                    .to_string()
            },
        });
    } else {
        checks.push(Check {
            level: Level::Ok,
            label: "GitHub CLI not installed (optional)".to_string(),
            detail: "Meow can still send when you paste a repository link.".to_string(),
        });
    }

    // Credential helper
    let helper = git_config("credential.helper");
    checks.push(Check {
        level: Level::Ok,
        label: match &helper {
            Some(helper) if !helper.trim().is_empty() => {
                format!("Login helper configured ({helper})")
            }
            _ => "No login helper (Git will use its default)".to_string(),
        },
        detail: String::new(),
    });

    // Project checks
    let cwd = std::env::current_dir().unwrap_or_default();
    match Repo::discover(&cwd, &Ui::new(true, true, true, false, false)) {
        Ok(Some(repo)) => project_checks(&repo, &mut checks),
        Ok(None) => checks.push(Check {
            level: Level::Ok,
            label: "Not inside a project".to_string(),
            detail: "Global checks still apply. Run Meow inside a project for more.".to_string(),
        }),
        Err(err) => checks.push(Check {
            level: Level::Warn,
            label: "This folder couldn't be inspected".to_string(),
            detail: format!("{err}"),
        }),
    }

    // Disk space (best effort; absent where it can't be read)
    #[cfg(any(unix, windows))]
    if let Some((free, total)) = disk_space(&cwd) {
        let ratio = if total > 0 {
            free as f64 / total as f64
        } else {
            1.0
        };
        checks.push(Check {
            level: if ratio < 0.03 { Level::Warn } else { Level::Ok },
            label: format!("{} free on this disk", human_size(free)),
            detail: if ratio < 0.03 {
                "Very little space left. Free some before the next big save.".to_string()
            } else {
                String::new()
            },
        });
    }

    checks
}

fn project_checks(repo: &Repo, checks: &mut Vec<Check>) {
    let label = display_path(&repo.root);
    checks.push(Check {
        level: Level::Ok,
        label: format!("Project at {label}"),
        detail: String::new(),
    });

    match repo.status() {
        Ok(status) => {
            if let Some(op) = status.operation {
                checks.push(Check {
                    level: Level::Warn,
                    label: format!("A {} is in progress", op.label()),
                    detail: if op == crate::repo::Operation::Merge {
                        "Finish it with `meow resolve`, or stop with `meow cancel`.".to_string()
                    } else {
                        format!("Meow doesn't manage this one. Finish it with `git {} --continue` or `git {} --abort`.", op.label(), op.label())
                    },
                });
            } else {
                checks.push(Check {
                    level: Level::Ok,
                    label: "No operation is half-finished".to_string(),
                    detail: String::new(),
                });
            }
        }
        Err(err) => checks.push(Check {
            level: Level::Fail,
            label: "The project status couldn't be read".to_string(),
            detail: format!("{err}"),
        }),
    }

    let remotes = repo.remotes();
    if remotes.is_empty() {
        checks.push(Check {
            level: Level::Ok,
            label: "No online home yet".to_string(),
            detail: "`meow send` can create one when you're ready.".to_string(),
        });
    } else {
        let home = repo.home_remote().unwrap_or_else(|| remotes[0].clone());
        checks.push(Check {
            level: Level::Ok,
            label: format!("Online home: {}", crate::repo::pretty_remote(&home.url)),
            detail: String::new(),
        });
    }

    if !repo.root.join(".gitignore").exists() {
        checks.push(Check {
            level: Level::Warn,
            label: "No .gitignore".to_string(),
            detail: "`meow ignore` can add one for installed packages, build output, and secrets."
                .to_string(),
        });
    }

    // Large files anywhere in history — these make clones and gets slow.
    if repo.has_commits() {
        let mut big = large_objects(repo, 5_000_000);
        big.sort_by_key(|(_, size)| std::cmp::Reverse(*size));
        if big.is_empty() {
            checks.push(Check {
                level: Level::Ok,
                label: "No very large files in your history".to_string(),
                detail: String::new(),
            });
        } else {
            let list = big
                .iter()
                .take(3)
                .map(|(p, s)| format!("{p} ({})", human_size(*s)))
                .collect::<Vec<_>>()
                .join(", ");
            checks.push(Check {
                level: Level::Warn,
                label: format!(
                    "{} in your history",
                    crate::util::count(big.len(), "large file")
                ),
                detail: format!("{list}. Large files make every get slow."),
            });
        }
    }

    if let Some(list) = repo.git.probe(&["stash", "list", "--format=%gs"]) {
        let mine = list
            .lines()
            .filter(|l| l.starts_with("meow: set aside"))
            .count();
        if mine > 0 {
            checks.push(Check {
                level: Level::Warn,
                label: format!(
                    "{} still set aside",
                    crate::util::count(mine, "set of unsaved changes")
                ),
                detail: "`meow resolve` puts them back after a combine. Before that, `git stash pop` does it directly."
                    .to_string(),
            });
        }
    }

    match Journal::load_strict(&repo.git_dir) {
        Ok(_) => checks.push(Check {
            level: Level::Ok,
            label: "Undo history is readable".to_string(),
            detail: String::new(),
        }),
        Err(err) => checks.push(Check {
            level: Level::Warn,
            label: "Undo history couldn't be read".to_string(),
            detail: format!("{err}. Your files and saves are unaffected."),
        }),
    }

    let snapshots = journal::list_snapshots(repo);
    let trash = journal::list_trash(&repo.git_dir);
    if !snapshots.is_empty() || !trash.is_empty() {
        checks.push(Check {
            level: Level::Ok,
            label: format!(
                "Safety copies: {} snapshot{}, {} trash entr{}",
                snapshots.len(),
                if snapshots.len() == 1 { "" } else { "s" },
                trash.len(),
                if trash.len() == 1 { "y" } else { "ies" }
            ),
            detail: "`meow recover` lists them. `meow recover --purge` removes them.".to_string(),
        });
    }

    #[cfg(unix)]
    {
        if let Some(git_dir) = repo.git_dir.to_str() {
            if let Ok(meta) = fs::metadata(git_dir) {
                use std::os::unix::fs::PermissionsExt;
                let mode = meta.permissions().mode() & 0o777;
                if mode & 0o002 != 0 {
                    checks.push(Check {
                        level: Level::Warn,
                        label: "Your .git folder is writable by everyone".to_string(),
                        detail: "Run `chmod -R go-w .git` to keep your history private."
                            .to_string(),
                    });
                }
            }
        }
    }
}

fn apply_fixes(ui: &Ui, checks: &[Check]) -> Result<i32> {
    let mut changed = false;
    if checks.iter().any(|c| c.label.starts_with("Name and email")) {
        if !ui.can_prompt() {
            return Err(ui.decision_required(
                "Meow needs a name and email to fix.",
                "Set them with: git config --global user.name \"Your Name\" and git config --global user.email \"you@example.com\"",
            ));
        }
        ui.blank();
        let name = ui.input("Your name", &git_config("user.name").unwrap_or_default())?;
        let email = ui.input("Your email", &git_config("user.email").unwrap_or_default())?;
        if name.trim().is_empty() || email.trim().is_empty() {
            ui.line("Nothing was changed.");
            return Ok(1);
        }
        let _ = Command::new("git")
            .args(["config", "--global", "user.name", name.trim()])
            .status();
        let _ = Command::new("git")
            .args(["config", "--global", "user.email", email.trim()])
            .status();
        ui.success("Saved your name and email.");
        changed = true;
    }
    if !changed {
        ui.line("There's nothing Meow can fix automatically.");
    }
    Ok(0)
}

fn write_report(checks: &[Check]) -> Result<PathBuf> {
    let dir = dirs::state_dir()
        .or_else(dirs::config_dir)
        .map(|d| d.join("meow"))
        .unwrap_or_else(std::env::temp_dir);
    fs::create_dir_all(&dir)?;
    let path = dir.join(format!("report-{}.txt", Utc::now().format("%Y%m%d-%H%M%S")));
    let mut body = String::new();
    body.push_str("meow doctor report\n");
    body.push_str(&format!("version: {}\n", env!("CARGO_PKG_VERSION")));
    body.push_str(&format!(
        "platform: {} {}\n",
        std::env::consts::OS,
        std::env::consts::ARCH
    ));
    body.push_str(&format!("when: {}\n\n", Utc::now().to_rfc3339()));
    for check in checks {
        let level = match check.level {
            Level::Ok => "ok",
            Level::Warn => "warn",
            Level::Fail => "fail",
        };
        body.push_str(&format!("[{level}] {}\n", redact(&check.label)));
        if !check.detail.is_empty() {
            body.push_str(&format!("       {}\n", redact(&check.detail)));
        }
    }
    fs::write(&path, body)?;
    Ok(path)
}

fn redact(text: &str) -> String {
    let mut out = text.to_string();
    if let Some(home) = dirs::home_dir() {
        out = out.replace(&home.display().to_string(), "~");
    }
    // Never include anything that looks like a token.
    for prefix in ["ghp_", "gho_", "ghu_", "ghs_", "github_pat_"] {
        while let Some(start) = out.find(prefix) {
            let end = out[start..]
                .find(|c: char| c.is_whitespace() || c == '"' || c == '\'')
                .map(|i| start + i)
                .unwrap_or(out.len());
            out.replace_range(start..end, "[redacted]");
        }
    }
    out
}

/// Blobs over `threshold` bytes anywhere in history, via one Git pipeline.
fn large_objects(repo: &Repo, threshold: u64) -> Vec<(String, u64)> {
    use std::io::Write as _;
    use std::process::Stdio;

    let Ok(rev) = Command::new("git")
        .current_dir(&repo.root)
        .args(["rev-list", "--objects", "--all"])
        .output()
    else {
        return Vec::new();
    };
    if !rev.status.success() {
        return Vec::new();
    }
    let Ok(mut child) = Command::new("git")
        .current_dir(&repo.root)
        .args([
            "cat-file",
            "--batch-check=%(objecttype) %(objectsize) %(rest)",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
    else {
        return Vec::new();
    };
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(&rev.stdout);
    }
    let Ok(out) = child.wait_with_output() else {
        return Vec::new();
    };
    let text = String::from_utf8_lossy(&out.stdout);
    let mut seen = std::collections::HashSet::new();
    let mut big = Vec::new();
    for line in text.lines() {
        let mut parts = line.splitn(3, ' ');
        let kind = parts.next().unwrap_or_default();
        let size = parts
            .next()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(0);
        let path = parts.next().unwrap_or_default();
        if kind == "blob" && size > threshold && !path.is_empty() && seen.insert(path.to_string()) {
            big.push((path.to_string(), size));
        }
    }
    big
}

fn git_config(key: &str) -> Option<String> {
    let out = Command::new("git")
        .args(["config", "--get", key])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let value = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

fn install_hint() -> String {
    if cfg!(target_os = "macos") {
        "Install with `brew install git`, or run `xcode-select --install`, then try again."
            .to_string()
    } else if cfg!(target_os = "windows") {
        "Install from git-scm.com/download/win, then reopen your terminal.".to_string()
    } else {
        "Install with your package manager, for example `sudo apt install git`.".to_string()
    }
}

#[cfg(windows)]
fn disk_space(path: &std::path::Path) -> Option<(u64, u64)> {
    let script = "$d = New-Object System.IO.DriveInfo($env:MEOW_DISK_PATH); Write-Output \"$($d.AvailableFreeSpace) $($d.TotalSize)\"";
    let out = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .env("MEOW_DISK_PATH", path)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let mut parts = text.split_whitespace();
    let free = parts.next()?.parse::<u64>().ok()?;
    let total = parts.next()?.parse::<u64>().ok()?;
    Some((free, total))
}

#[cfg(unix)]
fn disk_space(path: &std::path::Path) -> Option<(u64, u64)> {
    let out = Command::new("df").arg("-Pk").arg(path).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let line = text.lines().nth(1)?;
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() < 4 {
        return None;
    }
    let total = parts[1].parse::<u64>().ok()? * 1024;
    let free = parts[3].parse::<u64>().ok()? * 1024;
    Some((free, total))
}
