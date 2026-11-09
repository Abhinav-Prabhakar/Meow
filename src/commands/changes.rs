//! `meow changes` — review what actually changed before saving.

use anyhow::Result;

use crate::repo::{ChangeKind, Repo};
use crate::ui::Ui;
use crate::util::{author_label, relative_time, require_repo};

pub fn run(ui: &Ui, path: Option<&str>, stat: bool, saved: Option<&str>) -> Result<i32> {
    let repo = require_repo(ui)?;
    if let Some(n) = saved {
        return show_saved(ui, &repo, n, stat);
    }
    let status = repo.status()?;
    let files: Vec<_> = status
        .files
        .iter()
        .filter(|f| match path {
            Some(p) => f.path == p || f.path.starts_with(&format!("{}/", p.trim_end_matches('/'))),
            None => true,
        })
        .cloned()
        .collect();

    if files.is_empty() {
        if ui.plain {
            println!("changes 0 +0 -0");
            return Ok(0);
        }
        ui.title("Changes · none");
        ui.blank();
        ui.line("Nothing has changed since your last save.");
        if repo.has_commits() {
            if let Ok(info) = repo.head_info() {
                let me = repo.identity().map(|(n, _)| n);
                ui.line(ui.dim(format!(
                    "Last save: \"{}\" {} · {}",
                    info.subject,
                    author_label(&info.author, me.as_deref()),
                    relative_time(info.when)
                )));
            }
        }
        return Ok(0);
    }

    let stat_summary = repo.diff_stat();
    if ui.plain {
        println!(
            "changes {} +{} -{}",
            stat_summary.files, stat_summary.added, stat_summary.deleted
        );
    } else {
        ui.title(&format!(
            "Changes · {} · +{} -{}",
            crate::util::count(stat_summary.files, "file"),
            stat_summary.added,
            stat_summary.deleted
        ));
        ui.blank();
    }

    let color = if ui.color { "always" } else { "never" };
    let color_arg = format!("--color={color}");
    let mut body = String::new();

    if stat {
        let mut args = vec!["diff", "HEAD", "--stat", color_arg.as_str()];
        if let Some(p) = path {
            args.push("--");
            args.push(p);
        }
        if repo.has_commits() {
            if let Ok(out) = repo.git.check(&args) {
                body.push_str(&out.stdout);
            }
        }
        for file in files.iter().filter(|f| f.is_untracked()) {
            if file.kind == ChangeKind::Untracked {
                let lines = std::fs::read_to_string(repo.root.join(&file.path))
                    .map(|s| s.lines().count())
                    .unwrap_or(0);
                body.push_str(&format!(" {} | {} +\n", file.path, lines));
            }
        }
    } else {
        if repo.has_commits() {
            let mut args = vec!["diff", "HEAD", "-M", "--unified=3", color_arg.as_str()];
            if let Some(p) = path {
                args.push("--");
                args.push(p);
            }
            if let Ok(out) = repo.git.check(&args) {
                body.push_str(&out.stdout);
            }
        }
        for file in files.iter().filter(|f| f.is_untracked()) {
            body.push_str(&untracked_diff(&repo, &file.path, ui.color));
        }
    }

    if body.trim().is_empty() {
        ui.line("Those paths haven't changed.");
        return Ok(0);
    }

    if ui.plain {
        print!("{body}");
    } else {
        ui.page(&body);
        if ui.is_terminal() {
            ui.blank();
            ui.hint("q quit · meow save when you're happy");
        }
    }
    Ok(0)
}

fn untracked_diff(repo: &Repo, path: &str, color: bool) -> String {
    let color = if color { "always" } else { "never" };
    let full = repo.root.join(path);
    let out = repo.git.run(&[
        "diff",
        "--no-index",
        &format!("--color={color}"),
        "--",
        "/dev/null",
        &full.to_string_lossy(),
    ]);
    match out {
        Ok(out) if !out.stdout.is_empty() => {
            let mut text = out.stdout;
            // Make the header read like a normal new file.
            text = text.replace(&format!("b/{}", full.display()), &format!("b/{path}"));
            text
        }
        Ok(out) if !out.stderr.is_empty() => format!("New file: {path}\n"),
        _ => format!("New file: {path}\n"),
    }
}

fn show_saved(ui: &Ui, repo: &Repo, position: &str, stat: bool) -> Result<i32> {
    let n: usize = position
        .trim_start_matches('#')
        .parse()
        .map_err(|_| anyhow::anyhow!("Use a save number, like `meow changes --saved 3`."))?;
    if n == 0 {
        return Err(anyhow::anyhow!("Saves are counted from 1."));
    }
    let commits = repo.commits(n.max(1), false, None)?;
    let Some(commit) = commits.get(n - 1) else {
        ui.line(format!("There is no save #{n}."));
        return Ok(0);
    };
    let stats = repo.commit_stat(&commit.sha);
    if ui.plain {
        println!(
            "saved {} \"{}\" {}",
            commit.short,
            commit.subject,
            crate::util::count(stats.files, "file")
        );
    } else {
        ui.title(&format!("Save #{} · \"{}\"", n, commit.subject));
        ui.blank();
        ui.line(ui.dim(format!(
            "{} · {} · {} · +{} -{}",
            commit.author,
            relative_time(commit.when),
            crate::util::count(stats.files, "file"),
            stats.added,
            stats.deleted
        )));
        ui.blank();
    }
    let color = if ui.color { "always" } else { "never" };
    let color_arg = format!("--color={color}");
    let args = if stat {
        vec![
            "show",
            "--format=",
            "--stat",
            color_arg.as_str(),
            &commit.sha,
        ]
    } else {
        vec![
            "show",
            "--format=",
            "-M",
            "--unified=3",
            color_arg.as_str(),
            &commit.sha,
        ]
    };
    let out = repo.git.check(&args)?;
    if ui.plain {
        print!("{}", out.stdout);
    } else {
        ui.page(&out.stdout);
        ui.blank();
        ui.hint(format!("meow history {}   bring this version back", n));
    }
    Ok(0)
}
