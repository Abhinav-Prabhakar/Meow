//! `meow resolve` — fix conflicts one block at a time, in plain words.

use std::fs;
use std::path::Path;

use anyhow::Result;

use crate::repo::Repo;
use crate::ui::{display_width, pad_right, Sym, Ui};
use crate::util::{is_binary, open_editor, require_repo, truncate_chars};

pub fn run(ui: &Ui, ours: bool, theirs: bool, both: bool, list: bool) -> Result<i32> {
    let repo = require_repo(ui)?;
    let status = repo.status()?;
    let mode = conflict_mode(&repo, &status);

    if list {
        return show_list(ui, &status.conflicts);
    }

    if status.conflicts.is_empty() {
        if status.operation.is_some() {
            return finish(ui, &repo, 0, mode);
        }
        if ui.plain {
            println!("conflicts 0");
            if set_aside_stash(&repo).is_some() {
                println!("set-aside 1");
            }
        } else {
            ui.line(format!("{} Nothing needs resolving.", ui.sym(Sym::Success)));
            if set_aside_stash(&repo).is_some() {
                ui.blank();
                ui.line("Your unsaved changes are still set aside from an earlier get.");
                ui.next(&[("meow recover", "bring them back")]);
            }
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
            apply_side(&repo, path, side, mode)?;
        }
        return finish(ui, &repo, status.conflicts.len(), mode);
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

    show_intro(ui, &status.conflicts, &repo, mode)?;

    let mut deferred = Vec::new();
    for (i, path) in status.conflicts.iter().enumerate() {
        if !resolve_file(ui, &repo, path, i + 1, status.conflicts.len(), mode)? {
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
        if mode == Mode::SetAside {
            ui.line("Your unsaved changes are still set aside safely.");
        }
        ui.line("Nothing was lost. Run `meow resolve` when you're ready to continue.");
        return Ok(0);
    }

    finish(ui, &repo, status.conflicts.len(), mode)
}

/// Where the two conflicting versions came from.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// An ordinary combine of two branches.
    Merge,
    /// `meow get` set unsaved work aside and could not put it back cleanly.
    SetAside,
}

impl Mode {
    fn is_set_aside(self) -> bool {
        self == Mode::SetAside
    }
}

/// A combine is the common case. A conflict with no operation in progress and
/// a Meow set-aside stash is the overlap between unsaved work and new saves.
fn conflict_mode(repo: &Repo, status: &crate::repo::Status) -> Mode {
    if status.operation.is_some() || status.conflicts.is_empty() {
        return Mode::Merge;
    }
    if set_aside_stash(repo).is_some() {
        Mode::SetAside
    } else {
        Mode::Merge
    }
}

pub(crate) fn set_aside_stash(repo: &Repo) -> Option<String> {
    let list = repo.git.probe(&["stash", "list", "--format=%gd%x00%gs"])?;
    list.lines()
        .find(|line| line.contains("meow: set aside before get"))
        .and_then(|line| line.split('\0').next())
        .map(str::to_string)
}

#[derive(Clone, Copy)]
enum Side {
    Mine,
    Theirs,
    Both,
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

fn show_intro(ui: &Ui, conflicts: &[String], repo: &Repo, mode: Mode) -> Result<()> {
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
    if mode.is_set_aside() {
        ui.line("Your unsaved changes and the new saves touched the same parts.");
        ui.line("Nothing is lost. Your unsaved version is on the right.");
    } else {
        match merge_source(repo) {
            Some(name) => ui.line(format!(
                "You and {} changed the same parts. Nothing is lost.",
                ui.cyan(&name)
            )),
            None => ui.line("Two versions of these files exist. Nothing is lost."),
        }
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
fn resolve_file(
    ui: &Ui,
    repo: &Repo,
    path: &str,
    index: usize,
    total: usize,
    mode: Mode,
) -> Result<bool> {
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
                apply_side(repo, path, Side::Mine, mode)?;
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
                apply_side(repo, path, Side::Mine, mode)?;
                Ok(true)
            }
            1 => {
                apply_side(repo, path, Side::Theirs, mode)?;
                Ok(true)
            }
            _ => Ok(false),
        }
    } else {
        resolve_text(ui, repo, path, index, total, mode)
    }
}

fn resolve_text(
    ui: &Ui,
    repo: &Repo,
    path: &str,
    index: usize,
    total: usize,
    mode: Mode,
) -> Result<bool> {
    let full = repo.root.join(path);
    let content = fs::read_to_string(&full).unwrap_or_default();
    let mut parsed = parse(&content);
    if mode.is_set_aside() {
        swap_sides(&mut parsed);
    }
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
                    mode,
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
    mode: Mode,
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
    let (mine_label, theirs_label, mine_choice, theirs_choice) = if mode.is_set_aside() {
        (
            "Your unsaved version".to_string(),
            "The newer saved version".to_string(),
            "Keep my unsaved change".to_string(),
            "Take the newer saved version".to_string(),
        )
    } else {
        let theirs_label = if their_label.is_empty() {
            "Their version".to_string()
        } else {
            format!("{}'s version", their_label)
        };
        (
            "Your version".to_string(),
            theirs_label.clone(),
            "Keep mine".to_string(),
            format!("Take {theirs_label}"),
        )
    };
    for line in side_by_side(ui, &mine_label, &theirs_label, mine, theirs) {
        ui.line(line);
    }
    ui.blank();
    let choice = ui.select(
        "What should this part say?",
        &[
            mine_choice,
            theirs_choice,
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

fn apply_side(repo: &Repo, path: &str, side: Side, mode: Mode) -> Result<()> {
    // In a set-aside conflict, stage 2 is the newer saved work and stage 3 is
    // the unsaved work Meow set aside. "Mine" means the user's unsaved changes.
    let (mine_flag, theirs_flag) = if mode.is_set_aside() {
        ("--theirs", "--ours")
    } else {
        ("--ours", "--theirs")
    };
    match side {
        Side::Mine => {
            repo.git
                .check(&["checkout", mine_flag, "--", path])
                .or_else(|_| repo.git.check(&["checkout", "HEAD", "--", path]))?;
        }
        Side::Theirs => {
            repo.git.check(&["checkout", theirs_flag, "--", path])?;
        }
        Side::Both => {
            // Keep both sides *per conflict block*, so shared context above
            // and below the conflict is not duplicated.
            let full = repo.root.join(path);
            let content = fs::read_to_string(&full).unwrap_or_default();
            let mut parsed = parse(&content);
            if mode.is_set_aside() {
                swap_sides(&mut parsed);
            }
            let mut combined = String::new();
            for segment in parsed {
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

/// Stash conflicts put the newer saved work on the left and the unsaved work
/// on the right. Flip them so the labels match what the person chose.
fn swap_sides(segments: &mut [Segment]) {
    for segment in segments {
        if let Segment::Conflict {
            mine,
            theirs,
            label,
        } = segment
        {
            std::mem::swap(mine, theirs);
            if label.is_empty() || label == "Stashed changes" {
                *label = "the newer saved version".to_string();
            }
        }
    }
}

fn finish(ui: &Ui, repo: &Repo, resolved: usize, mode: Mode) -> Result<i32> {
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
        if mode.is_set_aside() {
            ui.line("Your unsaved changes are still set aside safely.");
        }
        return Ok(0);
    }

    if mode.is_set_aside() {
        // The resolved working tree now holds the parts of the unsaved work the
        // person kept. The stash copy is therefore redundant.
        if let Some(reference) = set_aside_stash(repo) {
            let _ = repo.git.run(&["stash", "drop", "--quiet", &reference]);
        }
        // Make the resolution look like ordinary unsaved work again.
        let _ = repo.git.run(&["reset", "--quiet"]);
        if ui.plain {
            println!("resolved {}", crate::util::count(resolved, "file"));
        } else {
            ui.success("Your unsaved changes are back in place.");
            ui.line(ui.dim("They're not saved yet — `meow save` when you're happy."));
        }
        return Ok(0);
    }

    let merging = repo.git_dir.join("MERGE_HEAD").exists();
    if !merging {
        if status.operation.is_some() {
            ui.line("All the conflicts are resolved. The operation will continue on its own.");
        } else if ui.plain {
            println!("resolved {}", crate::util::count(resolved, "file"));
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
    Ok(0)
}
