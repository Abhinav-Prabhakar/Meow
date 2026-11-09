//! Small helpers shared by commands.

use std::path::Path;
use std::process::Command;

use anyhow::{anyhow, Result};
use chrono::{DateTime, Datelike, FixedOffset, Utc};

use crate::repo::Repo;
use crate::ui::Ui;

pub fn require_repo(ui: &Ui) -> Result<Repo> {
    let cwd = std::env::current_dir()?;
    match Repo::discover(&cwd, ui)? {
        Some(repo) => Ok(repo),
        None => Err(anyhow!(
            "This folder isn't a project yet.\n  Start one with `meow new`, or copy one with `meow get <link>`."
        )),
    }
}

pub fn display_path(path: &Path) -> String {
    if let Some(home) = dirs::home_dir() {
        if let Ok(rest) = path.strip_prefix(&home) {
            if rest.as_os_str().is_empty() {
                return "~".to_string();
            }
            return format!("~/{}", rest.display());
        }
    }
    path.display().to_string()
}

pub fn relative_time(when: DateTime<FixedOffset>) -> String {
    relative_to(when.with_timezone(&Utc), Utc::now())
}

pub fn relative_to(when: DateTime<Utc>, now: DateTime<Utc>) -> String {
    let delta = now.signed_duration_since(when);
    if delta.num_seconds() < 45 {
        return "just now".to_string();
    }
    if delta.num_minutes() < 60 {
        let n = delta.num_minutes().max(1);
        return format!("{n} minute{} ago", plural(n));
    }
    if delta.num_hours() < 24 {
        let n = delta.num_hours();
        return format!("{n} hour{} ago", plural(n));
    }
    if delta.num_hours() < 48 {
        return "yesterday".to_string();
    }
    if delta.num_days() < 30 {
        let n = delta.num_days();
        return format!("{n} day{} ago", plural(n));
    }
    if delta.num_days() < 365 {
        let n = delta.num_days() / 7;
        return format!("{n} week{} ago", plural(n));
    }
    let n = delta.num_days() / 365;
    format!("{n} year{} ago", plural(n))
}

/// `1 file` / `3 files` — the small grammar that makes output feel human.
pub fn count(n: usize, singular: &str) -> String {
    if n == 1 {
        format!("{n} {singular}")
    } else {
        format!("{n} {singular}s")
    }
}

fn plural(n: i64) -> &'static str {
    if n == 1 {
        ""
    } else {
        "s"
    }
}

pub fn day_label(when: DateTime<FixedOffset>) -> String {
    let local = when.with_timezone(&chrono::Local);
    let today = chrono::Local::now().date_naive();
    let date = local.date_naive();
    if date == today {
        "Today".to_string()
    } else if Some(date) == today.pred_opt() {
        "Yesterday".to_string()
    } else if date.year() == today.year() {
        format!("{} {}", month_name(date.month()), date.day())
    } else {
        format!(
            "{} {}, {}",
            month_name(date.month()),
            date.day(),
            date.year()
        )
    }
}

fn month_name(m: u32) -> &'static str {
    match m {
        1 => "January",
        2 => "February",
        3 => "March",
        4 => "April",
        5 => "May",
        6 => "June",
        7 => "July",
        8 => "August",
        9 => "September",
        10 => "October",
        11 => "November",
        12 => "December",
        _ => "",
    }
}

pub fn author_label(name: &str, me: Option<&str>) -> String {
    if let Some(me) = me {
        if name == me {
            return "you".to_string();
        }
    }
    name.split_whitespace().next().unwrap_or(name).to_string()
}

pub fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1000.0 && unit < UNITS.len() - 1 {
        value /= 1000.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{} {}", bytes, UNITS[unit])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

pub fn slugify(name: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;
    for ch in name.trim().chars() {
        let c = ch.to_ascii_lowercase();
        if c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '/' {
            out.push(c);
            last_dash = false;
        } else if (c == '-' || c.is_whitespace()) && !last_dash && !out.is_empty() {
            out.push('-');
            last_dash = true;
        }
    }
    let trimmed = out.trim_matches(['-', '/', '.']).to_string();
    let mut cleaned = trimmed;
    while cleaned.contains("--") {
        cleaned = cleaned.replace("--", "-");
    }
    if cleaned.len() > 60 {
        cleaned.truncate(60);
        cleaned = cleaned.trim_end_matches('-').to_string();
    }
    cleaned
}

pub fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0; b.len() + 1];
    for i in 1..=a.len() {
        cur[0] = i;
        for j in 1..=b.len() {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            cur[j] = (prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}

pub fn is_binary(path: &Path) -> bool {
    use std::io::Read;
    let Ok(mut file) = std::fs::File::open(path) else {
        return false;
    };
    let mut buf = [0u8; 8000];
    let Ok(n) = file.read(&mut buf) else {
        return false;
    };
    buf[..n].contains(&0)
}

pub fn open_editor(ui: &Ui, path: &Path) -> Result<()> {
    let config = crate::config::Config::load();
    let editor = config.editor();
    let mut parts = editor.split_whitespace();
    let Some(bin) = parts.next() else {
        return Ok(());
    };
    let status = Command::new(bin)
        .args(parts)
        .arg(path)
        .status()
        .map_err(|e| anyhow!("Couldn't open your editor ({bin}): {e}"))?;
    if !status.success() {
        ui.warning("Your editor closed with a problem. The file is unchanged.");
    }
    Ok(())
}

/// A deterministic, human note from the files being saved.
pub fn suggest_note(files: &[crate::repo::FileChange]) -> String {
    use crate::repo::ChangeKind;
    if files.is_empty() {
        return "Update files".to_string();
    }
    if files.iter().all(|f| f.kind == ChangeKind::Added) {
        if files.len() == 1 {
            return format!("Add {}", short_name(&files[0].path));
        }
        return format!("Add {} files", files.len());
    }
    if files.iter().all(|f| f.kind == ChangeKind::Deleted) {
        if files.len() == 1 {
            return format!("Remove {}", short_name(&files[0].path));
        }
        return format!("Remove {} files", files.len());
    }
    if files.len() == 1 {
        return format!("Update {}", short_name(&files[0].path));
    }
    format!("Update {} files", files.len())
}

fn short_name(path: &str) -> String {
    Path::new(path)
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string())
}

pub fn truncate_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
}
