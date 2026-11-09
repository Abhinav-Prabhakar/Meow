//! `meow version` — what you're running, and what it runs on.

use anyhow::Result;

use crate::ui::Ui;
use crate::util::display_path;

pub fn run(ui: &Ui, verbose: bool) -> Result<i32> {
    let meow = env!("CARGO_PKG_VERSION");
    let git = git_version().unwrap_or_else(|| "not found".to_string());
    let platform = format!("{} {}", std::env::consts::OS, std::env::consts::ARCH);

    if ui.plain {
        println!("meow {meow}");
        println!("git {git}");
        println!("platform {platform}");
        if verbose {
            if let Some(path) = crate::config::Config::path() {
                println!("config {}", path.display());
            }
            if let Some(dir) = dirs::state_dir() {
                println!("state {}", dir.join("meow").display());
            }
            println!("shell {}", std::env::var("SHELL").unwrap_or_default());
        }
        return Ok(0);
    }

    ui.title(&format!("meow {meow}"));
    ui.blank();
    ui.kv("meow", meow);
    ui.kv("git", git);
    ui.kv("platform", platform);
    if verbose {
        if let Some(path) = crate::config::Config::path() {
            ui.kv("config", display_path(&path));
        }
        if let Some(dir) = dirs::state_dir() {
            ui.kv("state", display_path(&dir.join("meow")));
        }
        if let Ok(cwd) = std::env::current_dir() {
            ui.kv("folder", display_path(&cwd));
        }
        ui.kv("shell", std::env::var("SHELL").unwrap_or_default());
    }
    ui.blank();
    ui.line(ui.dim("Upgrade through the package manager you installed Meow with."));
    Ok(0)
}

fn git_version() -> Option<String> {
    let out = std::process::Command::new("git")
        .arg("--version")
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    Some(text.trim().replace("git version ", ""))
}
