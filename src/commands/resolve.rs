//! `meow resolve` — fix conflicts one block at a time, in plain words.

use std::fs;
use std::path::Path;

use anyhow::Result;
use serde_json::json;

use crate::journal::{self, Action, Journal};
use crate::repo::Repo;
use crate::ui::{display_width, pad_right, Sym, Ui};
use crate::util::{is_binary, open_editor, require_repo, truncate_chars};

pub fn run(ui: &Ui, ours: bool, theirs: bool, both: bool, list: bool) -> Result<i32> {
    let repo = require_repo(ui)?;
    let status = repo.status()?;

    if list {
        return show_list(ui, &status.conflicts);
    }

    if status.conflicts.is_empty() {
        if status.operation.is_some() {
            return finish(ui, &repo, 0);
        }
        if ui.plain {
            println!("conflicts 0");
        } else {
            ui.line(format!("{} Nothing needs resolving.", ui.sym(Sym::Success)));
        }
        return Ok(0);
    }

    if ours || theirs || both {
        let side = if ours {
            Side::Mine
        } else if theirs {
            Side::Theirs
        } else {
            Side::Both
        };
        for path in &status.conflicts {
            apply_side(&repo, path, side)?;
        }
        if ui.plain {
            println!("resolved {} files", status.conflicts.len());
        } else {
            ui.success(format!(
                "Resolved {} {}.",
                status.conflicts.len(),
                if status.conflicts.len() == 1 {
                    "file"
                } else {
                    "files"
                }
            ));
        }
        record(&repo, &status.conflicts, side.label())?;
        return finish(ui, &repo, status.conflicts.len());
    }

    if !ui.can_prompt() {
        if ui.plain {
            for path in &status.conflicts {
                println!("conflict {path}");
            }
        } else {
            show_list(ui, &status.conflicts)?;
        }
        return Err(ui.decision_required(
            "Conflicts need a decision.",
            "Pass --ours, --theirs, or --both to resolve them all one way.",
        ));
    }

    show_intro(ui, &status.conflicts, &repo)?;

    let mut deferred = Vec::new();
    for (i, path) in status.conflicts.iter().enumerate() {
        if !resolve_file(ui, &repo, path, i + 1, status.conflicts.len())? {
            deferred.push(path.clone());
        }
    }

    if !deferred.is_empty() {
        ui.blank();
        ui.warning(format!(
            "{} {} still need help.",
            deferred.len(),
            if deferred.len() == 1 { "file" } else { "files" }
        ));
        ui.line("Nothing was lost. Run `meow resolve` when you're ready to continue.");
        return Ok(0);
    }

    finish(ui, &repo, status.conflicts.len())
}

#[derive(Clone, Copy)]
enum Side {
    Mine,
    Theirs,
    Both,
}

impl Side {
    fn label(self) -> &'static str {
        match self {
            Side::Mine => "kept your version",
            Side::Theirs => "took their version",
            Side::Both => "kept both versions",
        }
    }
}

fn show_list(ui: &Ui, conflicts: &[String]) -> Result<i32> {
    if ui.plain {
        for path in conflicts {
            println!("conflict {path}");
        }
        return Ok(0);
    }
    if conflicts.is_empty() {
        ui.line(format!("{} Nothing needs resolving.", ui.sym(Sym::Success)));
        return Ok(0);
    }
    ui.title(&format!(
        "{} {} your help",
        crate::util::count(conflicts.len(), "file"),
        if conflicts.len() == 1 {
            "needs"
        } else {
            "need"
        }
    ));
    ui.blank();
    for path in conflicts {
        ui.line(format!("  {}  {}", ui.yellow("!"), path));
    }
    ui.blank();
    ui.hint("meow resolve to fix them one at a time");
    Ok(0)
}

fn show_intro(ui: &Ui, conflicts: &[String], repo: &Repo) -> Result<()> {
    let other = merge_source(repo);
    ui.title(&format!(
        "{} {} your help",
        crate::util::count(conflicts.len(), "file"),
        if conflicts.len() == 1 {
            "needs"
        } else {
            "need"
        }
    ));
    ui.blank();
    match other {
        Some(name) => ui.line(format!(
            "You and {} changed the same parts. Nothing is lost.",
            ui.cyan(&name)
        )),
        None => ui.line("Two versions of these files exist. Nothing is lost."),
    }
    ui.line("Let's go through them one at a time.");
    ui.blank();
    Ok(())
}

fn merge_source(repo: &Repo) -> Option<String> {
    let text = fs::read_to_string(repo.git_dir.join("MERGE_MSG")).ok()?;
    let first = text.lines().next()?;
    let start = first.find('\'')?;
    let rest = &first[start + 1..];
    let end = rest.find('\'')?;
    Some(rest[..end].to_string())
}

/// Returns true when the file is fully resolved.
fn resolve_file(ui: &Ui, repo: &Repo, path: &str, index: usize, total: usize) -> Result<bool> {
    let full = repo.root.join(path);
    if !full.exists() {
        // Deleted on one side: the user still chooses.
        ui.blank();
        ui.line(format!("File {index} of {total} · {path}"));
        ui.line("This file was deleted on one side and changed on the other.");
        let choice = ui.select(
            "What should happen?",
            &[
                "Keep the file".to_string(),
                "Delete it".to_string(),
                "Decide later".to_string(),
            ],
            0,
        )?;
        match choice {
            0 => {
                apply_side(repo, path, Side::Mine)?;
                return Ok(true);
            }
            1 => {
                let _ = repo.git.run(&["rm", "--", path]);
                return Ok(true);
            }
            _ => return Ok(false),
        }
    }

    if is_binary(&full) {
        ui.blank();
        ui.line(format!("File {index} of {total} · {path}"));
        let size = fs::metadata(&full)
            .map(|m| crate::util::human_size(m.len()))
            .unwrap_or_default();
        ui.line(format!(
            "{} This file can't be merged line by line. It's {}.",
            ui.yellow("!"),
            size
        ));
        let choice = ui.select(
            "Which version should be kept?",
            &[
                "Mine".to_string(),
                "Theirs".to_string(),
                "Decide later".to_string(),
            ],
            0,
        )?;
        match choice {
            0 => {
                apply_side(repo, path, Side::Mine)?;
                Ok(true)
            }
            1 => {
                apply_side(repo, path, Side::Theirs)?;
                Ok(true)
            }
            _ => Ok(false),
        }
    } else {
        resolve_text(ui, repo, path, index, total)
    }
}

fn resolve_text(ui: &Ui, repo: &Repo, path: &str, index: usize, total: usize) -> Result<bool> {
    let full = repo.root.join(path);
    let content = fs::read_to_string(&full).unwrap_or_default();
    let parsed = parse(&content);
    let conflict_count = parsed
        .iter()
        .filter(|s| matches!(s, Segment::Conflict { .. }))
        .count();
    if conflict_count == 0 {
        // Already resolved by hand — just mark it done.
        repo.git.check(&["add", "--", path])?;
        return Ok(true);
    }

    let mut out = String::new();
    let mut number = 0;
    for segment in parsed {
        match segment {
            Segment::Text(text) => out.push_str(&text),
            Segment::Conflict {
                mine,
                theirs,
                label,
            } => {
                number += 1;
                let keep = ask_block(
                    ui,
                    path,
                    index,
                    total,
                    number,
                    conflict_count,
                    &mine,
                    &theirs,
                    &label,
                )?;
                match keep {
                    BlockChoice::Mine => push_lines(&mut out, &mine),
                    BlockChoice::Theirs => push_lines(&mut out, &theirs),
                    BlockChoice::Both => {
                        push_lines(&mut out, &mine);
                        out.push_str(&both_separator(path));
                        push_lines(&mut out, &theirs);
                    }
                    BlockChoice::Editor => {
                        open_editor(ui, &full)?;
                        let edited = fs::read_to_string(&full).unwrap_or(content.clone());
                        if edited.contains("<<<<<<<") {
                            ui.warning("The conflict markers are still in the file.");
                            if !ui.confirm("Mark it resolved anyway?", false)? {
                                return Ok(false);
                            }
                        }
                        repo.git.check(&["add", "--", path])?;
                        ui.success(format!("{path} is resolved."));
                        return Ok(true);
                    }
                    BlockChoice::Later => return Ok(false),
                }
            }
        }
    }

    fs::write(&full, out)?;
    repo.git.check(&["add", "--", path])?;
    ui.success(format!("{path} is resolved."));
    ui.blank();
    let mut action = Action::new("resolve", format!("Resolved {path}"));
    action.data = json!({ "path": path });
    let mut journal = Journal::load(&repo.git_dir);
    let _ = journal.record(&repo.git_dir, action);
    Ok(true)
}

#[derive(Clone, Copy)]
enum BlockChoice {
    Mine,
    Theirs,
    Both,
    Editor,
    Later,
}

#[allow(clippy::too_many_arguments)]
fn ask_block(
    ui: &Ui,
    path: &str,
    file_index: usize,
    file_total: usize,
    block: usize,
    blocks: usize,
    mine: &[String],
    theirs: &[String],
    their_label: &str,
) -> Result<BlockChoice> {
    ui.blank();
    if blocks == 1 {
        ui.line(format!("File {file_index} of {file_total} · {path}"));
    } else {
        ui.line(format!(
            "File {file_index} of {file_total} · {path} · part {block} of {blocks}"
        ));
    }
    ui.blank();
    let mine_label = "Your version";
    let theirs_label = if their_label.is_empty() {
        "Their version".to_string()
    } else {
        format!("{}'s version", their_label)
    };
    for line in side_by_side(ui, mine_label, &theirs_label, mine, theirs) {
        ui.line(line);
    }
    ui.blank();
    let choice = ui.select(
        "What should this part say?",
        &[
            "Keep mine".to_string(),
            format!("Take {theirs_label}"),
            "Keep both".to_string(),
            "Open in my editor".to_string(),
            "Decide later".to_string(),
        ],
        0,
    )?;
    Ok(match choice {
        0 => BlockChoice::Mine,
        1 => BlockChoice::Theirs,
        2 => BlockChoice::Both,
        3 => BlockChoice::Editor,
        _ => BlockChoice::Later,
    })
}

fn side_by_side(
    ui: &Ui,
    left_label: &str,
    right_label: &str,
    left: &[String],
    right: &[String],
) -> Vec<String> {
    let width = ui.width().min(100).saturating_sub(6);
    let col = width / 2;
    let take = 6;
    let mut out = Vec::new();
    out.push(format!(
        "  {}{}{}",
        ui.bold(pad_right(left_label, col)),
        ui.dim("│"),
        ui.bold(right_label)
    ));
    for i in 0..take {
        let l = left
            .get(i)
            .map(|s| truncate_chars(s, col.saturating_sub(2)))
            .unwrap_or_default();
        let r = right
            .get(i)
            .map(|s| truncate_chars(s, col.saturating_sub(2)))
            .unwrap_or_default();
        out.push(format!(
            "  {}{}{}",
            ui.dim(pad_right(&l, col)),
            ui.dim("│"),
            ui.dim(r)
        ));
    }
    let hidden = left.len().saturating_sub(take) + right.len().saturating_sub(take);
    if hidden > 0 {
        out.push(format!(
            "  {}",
            ui.dim(format!(
                "… {hidden} more lines — open in your editor to see everything"
            ))
        ));
    }
    let _ = display_width("");
    out
}

enum Segment {
    Text(String),
    Conflict {
        mine: Vec<String>,
        theirs: Vec<String>,
        label: String,
    },
}

fn parse(content: &str) -> Vec<Segment> {
    let mut segments = Vec::new();
    let mut text: Vec<String> = Vec::new();
    let mut lines = content.lines().peekable();
    while let Some(line) = lines.next() {
        if line.starts_with("<<<<<<<") {
            let mut mine = Vec::new();
            let mut theirs = Vec::new();
            let mut label = String::new();
            // Ours
            for l in lines.by_ref() {
                if l.starts_with("=======") {
                    break;
                }
                if l.starts_with("|||||||") {
                    // diff3 base section — skip it.
                    for b in lines.by_ref() {
                        if b.starts_with("=======") {
                            break;
                        }
                    }
                    break;
                }
                mine.push(l.to_string());
            }
            // Theirs
            for l in lines.by_ref() {
                if let Some(rest) = l.strip_prefix(">>>>>>>") {
                    label = rest.trim().to_string();
                    break;
                }
                theirs.push(l.to_string());
            }
            if !text.is_empty() {
                let mut chunk = text.join("\n");
                chunk.push('\n');
                segments.push(Segment::Text(chunk));
                text.clear();
            }
            segments.push(Segment::Conflict {
                mine,
                theirs,
                label,
            });
        } else {
            text.push(line.to_string());
        }
    }
    if !text.is_empty() {
        let mut chunk = text.join("\n");
        if content.ends_with('\n') {
            chunk.push('\n');
        }
        segments.push(Segment::Text(chunk));
    }
    segments
}

fn push_lines(out: &mut String, lines: &[String]) {
    for line in lines {
        out.push_str(line);
        out.push('\n');
    }
}

fn both_separator(path: &str) -> String {
    let ext = Path::new(path)
        .extension()
        .map(|s| s.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "rs" | "js" | "jsx" | "ts" | "tsx" | "c" | "h" | "cpp" | "hpp" | "go" | "java"
        | "swift" | "kt" | "kts" | "cs" | "php" => "// meow: kept both versions\n".to_string(),
        "py" | "rb" | "sh" | "bash" | "zsh" | "yaml" | "yml" | "toml" | "r" | "pl" => {
            "# meow: kept both versions\n".to_string()
        }
        "html" | "htm" | "xml" | "md" | "svg" | "vue" | "svelte" => {
            "<!-- meow: kept both versions -->\n".to_string()
        }
        "sql" | "lua" => "-- meow: kept both versions\n".to_string(),
        _ => "meow: kept both versions\n".to_string(),
    }
}

fn apply_side(repo: &Repo, path: &str, side: Side) -> Result<()> {
    match side {
        Side::Mine => {
            repo.git
                .check(&["checkout", "--ours", "--", path])
                .or_else(|_| repo.git.check(&["checkout", "HEAD", "--", path]))?;
        }
        Side::Theirs => {
            repo.git.check(&["checkout", "--theirs", "--", path])?;
        }
        Side::Both => {
            // Keep both sides *per conflict block*, so shared context above
            // and below the conflict is not duplicated.
            let full = repo.root.join(path);
            let content = fs::read_to_string(&full).unwrap_or_default();
            let mut combined = String::new();
            for segment in parse(&content) {
                match segment {
                    Segment::Text(text) => combined.push_str(&text),
                    Segment::Conflict { mine, theirs, .. } => {
                        push_lines(&mut combined, &mine);
                        combined.push_str(&both_separator(path));
                        push_lines(&mut combined, &theirs);
                    }
                }
            }
            fs::write(&full, combined)?;
        }
    }
    repo.git.check(&["add", "--", path])?;
    Ok(())
}

fn record(repo: &Repo, paths: &[String], how: &str) -> Result<()> {
    let mut action = Action::new("resolve", format!("Resolved {} files ({how})", paths.len()));
    action.data = json!({ "paths": paths, "how": how });
    let mut journal = Journal::load(&repo.git_dir);
    let _ = journal.record(&repo.git_dir, action);
    Ok(())
}

fn finish(ui: &Ui, repo: &Repo, resolved: usize) -> Result<i32> {
    let status = repo.status()?;
    if status.conflicted() {
        ui.blank();
        ui.warning(format!(
            "{} {} still need help.",
            status.conflicts.len(),
            if status.conflicts.len() == 1 {
                "file"
            } else {
                "files"
            }
        ));
        return Ok(0);
    }
    let merging = repo.git_dir.join("MERGE_HEAD").exists();
    if !merging {
        if status.operation.is_some() {
            ui.line("All the conflicts are resolved. The operation will continue on its own.");
        } else if ui.plain {
            println!("resolved {resolved} files");
        } else {
            ui.line(format!(
                "{} All conflicts are resolved.",
                ui.sym(Sym::Success)
            ));
        }
        return Ok(0);
    }

    if !ui.plain {
        ui.blank();
        ui.line("Everything is combined. Finishing creates one save with both sides.");
    }
    if ui.can_prompt() && !ui.confirm("Finish the combine?", true)? {
        ui.line("The combine is still open. `meow resolve` when you're ready.");
        return Ok(0);
    }
    let out = repo.git.run_main(&["commit", "--no-edit"])?;
    if !out.success() {
        ui.failure("The combine couldn't be finished.");
        for line in out.stderr.lines().take(14) {
            ui.line(ui.dim(line));
        }
        return Ok(1);
    }
    let mut action = Action::new(
        "merge-finish",
        "Finished a combine after resolving conflicts",
    );
    action.head_before = None;
    action.head_after = repo.head();
    let mut journal = Journal::load(&repo.git_dir);
    let _ = journal.record(&repo.git_dir, action);

    if ui.plain {
        println!("combined finished");
        return Ok(0);
    }
    ui.success("Combined. Everything is in one place again.");

    // `meow get` sets unsaved work aside before bringing in new saves. Once the
    // combine is finished, put it back — and never fail silently.
    if let Some(list) = repo.git.probe(&["stash", "list", "--format=%gd%x00%gs"]) {
        let stashed = list
            .lines()
            .find(|line| line.contains("meow: set aside before get"))
            .and_then(|line| line.split('\0').next())
            .map(str::to_string);
        if let Some(reference) = stashed {
            let pop = repo.git.run(&["stash", "pop", "--quiet", &reference])?;
            if pop.success() {
                ui.success("Your unsaved changes are back in place.");
            } else {
                ui.warning("Your unsaved changes are still set aside safely.");
                ui.line("Run `meow recover` to bring them back.");
            }
        }
    }

    let status = repo.status()?;
    if crate::commands::home::unsent_count(repo, &status) > 0 && repo.home_label().is_some() {
        ui.next(&[("meow send", "put the combined work online")]);
    }
    let _ = journal::create_snapshot(repo, "After resolving conflicts");
    Ok(0)
}
