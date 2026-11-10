//! Private local state: the action journal, safety snapshots, and the trash.
//!
//! Everything here lives under `.git/meow/` and `refs/meow/*`. It is never
//! committed, never pushed, and deleting it loses undo history but not work.

use std::ffi::OsString;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::repo::Repo;

pub const JOURNAL_VERSION: u32 = 1;
const MAX_ACTIONS: usize = 50;
const KEEP_SNAPSHOTS: usize = 20;
const KEEP_DAYS: i64 = 30;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Action {
    pub id: String,
    pub kind: String,
    pub at: DateTime<Utc>,
    pub summary: String,
    #[serde(default)]
    pub head_before: Option<String>,
    #[serde(default)]
    pub head_after: Option<String>,
    #[serde(default)]
    pub snapshot: Option<String>,
    #[serde(default)]
    pub data: serde_json::Value,
}

impl Action {
    pub fn new(kind: &str, summary: impl Into<String>) -> Self {
        Action {
            id: new_id(),
            kind: kind.to_string(),
            at: Utc::now(),
            summary: summary.into(),
            head_before: None,
            head_after: None,
            snapshot: None,
            data: json!({}),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Journal {
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(default)]
    pub actions: Vec<Action>,
    #[serde(default)]
    pub redo: Vec<Action>,
}

fn default_version() -> u32 {
    JOURNAL_VERSION
}

impl Journal {
    pub fn load(git_dir: &Path) -> Journal {
        let path = journal_path(git_dir);
        let Ok(raw) = fs::read_to_string(&path) else {
            return Journal {
                version: JOURNAL_VERSION,
                ..Default::default()
            };
        };
        match serde_json::from_str::<Journal>(&raw) {
            Ok(mut j) => {
                if j.version == 0 {
                    j.version = JOURNAL_VERSION;
                }
                j
            }
            Err(_) => {
                let _ = fs::rename(&path, path.with_extension("corrupt"));
                Journal {
                    version: JOURNAL_VERSION,
                    ..Default::default()
                }
            }
        }
    }

    /// Load, but tell the user honestly when the journal can't be read.
    pub fn load_strict(git_dir: &Path) -> Result<Journal> {
        let path = journal_path(git_dir);
        if !path.exists() {
            return Ok(Journal {
                version: JOURNAL_VERSION,
                ..Default::default()
            });
        }
        let raw = fs::read_to_string(&path).context("Couldn't read the action journal")?;
        let journal: Journal =
            serde_json::from_str(&raw).context("The action journal couldn't be understood")?;
        Ok(journal)
    }

    pub fn save(&self, git_dir: &Path) -> Result<()> {
        let path = journal_path(git_dir);
        write_private(&path, serde_json::to_vec_pretty(self)?)
    }

    pub fn record(&mut self, git_dir: &Path, action: Action) -> Result<()> {
        self.actions.push(action);
        if self.actions.len() > MAX_ACTIONS {
            let drain = self.actions.len() - MAX_ACTIONS;
            self.actions.drain(0..drain);
        }
        self.redo.clear();
        self.save(git_dir)
    }
}

pub fn journal_path(git_dir: &Path) -> PathBuf {
    git_dir.join("meow").join("journal.json")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotMeta {
    pub id: String,
    pub sha: String,
    pub label: String,
    pub at: DateTime<Utc>,
    pub files: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SnapshotIndex {
    #[serde(default)]
    pub snapshots: Vec<SnapshotMeta>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrashMeta {
    pub id: String,
    pub label: String,
    pub at: DateTime<Utc>,
    pub files: Vec<String>,
}

pub fn meow_dir(git_dir: &Path) -> PathBuf {
    git_dir.join("meow")
}

pub fn ensure_meow_dir(git_dir: &Path) -> Result<PathBuf> {
    let dir = meow_dir(git_dir);
    fs::create_dir_all(&dir)?;
    restrict(&dir);
    Ok(dir)
}

pub fn new_id() -> String {
    let now = Utc::now();
    format!(
        "{}-{}-{}",
        now.format("%Y%m%d%H%M%S"),
        now.timestamp_subsec_millis(),
        std::process::id()
    )
}

fn write_private(path: &Path, bytes: Vec<u8>) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
        restrict(parent);
    }
    let tmp = path.with_extension(format!("tmp-{}", std::process::id()));
    {
        let mut file = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&tmp)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = file.set_permissions(fs::Permissions::from_mode(0o600));
        }
        file.write_all(&bytes)?;
        file.sync_all().ok();
    }
    fs::rename(&tmp, path)?;
    Ok(())
}

#[cfg(unix)]
fn restrict(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o700));
}

#[cfg(not(unix))]
fn restrict(_path: &Path) {}

// --------------------------------------------------------------------- snapshots

pub fn create_snapshot(repo: &Repo, label: &str) -> Result<String> {
    let dir = ensure_meow_dir(&repo.git_dir)?;
    let id = new_id();
    let index = dir.join(format!("tmp-index-{id}"));
    let index_os: OsString = index.clone().into_os_string();
    let env = [("GIT_INDEX_FILE", index_os.as_os_str())];

    let has_head = repo.has_commits();
    if has_head {
        let out = repo
            .git
            .run_env(&["read-tree", "HEAD"], &env)
            .context("Couldn't prepare a safety snapshot")?;
        if !out.success() {
            return Err(anyhow!("Couldn't prepare a safety snapshot"));
        }
    } else {
        let out = repo
            .git
            .run_env(&["read-tree", "--empty"], &env)
            .context("Couldn't prepare a safety snapshot")?;
        if !out.success() {
            return Err(anyhow!("Couldn't prepare a safety snapshot"));
        }
    }

    let added = repo
        .git
        .run_env(&["add", "-A"], &env)
        .context("Couldn't read the working tree for a safety snapshot")?;
    if !added.success() {
        let _ = fs::remove_file(&index);
        return Err(anyhow!(
            "Couldn't read the working tree for a safety snapshot"
        ));
    }

    let tree = repo
        .git
        .run_env(&["write-tree"], &env)
        .context("Couldn't write a safety snapshot")?;
    let _ = fs::remove_file(&index);
    if !tree.success() {
        return Err(anyhow!("Couldn't write a safety snapshot"));
    }
    let tree = tree.stdout.trim().to_string();

    let author_name = std::ffi::OsString::from("meow");
    let author_email = std::ffi::OsString::from("meow@localhost");
    let message = format!("meow snapshot: {label}");
    let mut args: Vec<&str> = vec!["commit-tree", &tree];
    let head;
    if has_head {
        head = repo.head().unwrap_or_default();
        if !head.is_empty() {
            args.push("-p");
            args.push(&head);
        }
    }
    args.push("-m");
    args.push(&message);
    let commit = repo.git.run_env(
        &args,
        &[
            ("GIT_AUTHOR_NAME", author_name.as_os_str()),
            ("GIT_AUTHOR_EMAIL", author_email.as_os_str()),
            ("GIT_COMMITTER_NAME", author_name.as_os_str()),
            ("GIT_COMMITTER_EMAIL", author_email.as_os_str()),
        ],
    )?;
    if !commit.success() {
        return Err(anyhow!("Couldn't record a safety snapshot"));
    }
    let sha = commit.stdout.trim().to_string();
    let refname = format!("refs/meow/snapshots/{id}");
    repo.git.check(&["update-ref", &refname, &sha])?;
    let files = repo
        .git
        .probe(&["ls-tree", "-r", "--name-only", &sha])
        .map(|s| s.lines().count())
        .unwrap_or(0);

    let mut idx = load_snapshots(&repo.git_dir);
    idx.snapshots.push(SnapshotMeta {
        id,
        sha: sha.clone(),
        label: label.to_string(),
        at: Utc::now(),
        files,
    });
    save_snapshots(&repo.git_dir, &idx)?;
    prune_snapshots(repo, &mut idx);
    Ok(sha)
}

pub fn load_snapshots(git_dir: &Path) -> SnapshotIndex {
    let path = meow_dir(git_dir).join("snapshots.json");
    fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_snapshots(git_dir: &Path, idx: &SnapshotIndex) -> Result<()> {
    let path = meow_dir(git_dir).join("snapshots.json");
    write_private(&path, serde_json::to_vec_pretty(idx)?)
}

pub fn list_snapshots(repo: &Repo) -> Vec<SnapshotMeta> {
    let mut idx = load_snapshots(&repo.git_dir);
    idx.snapshots.sort_by_key(|s| std::cmp::Reverse(s.at));
    idx.snapshots
        .retain(|s| repo.git.probe(&["cat-file", "-e", &s.sha]).is_some());
    idx.snapshots
}

fn prune_snapshots(repo: &Repo, idx: &mut SnapshotIndex) {
    idx.snapshots.sort_by_key(|s| std::cmp::Reverse(s.at));
    let cutoff = Utc::now() - Duration::days(KEEP_DAYS);
    let mut kept = Vec::new();
    for (i, snap) in idx.snapshots.iter().enumerate() {
        if i < KEEP_SNAPSHOTS || snap.at > cutoff {
            kept.push(snap.clone());
        } else {
            let _ = repo.git.run(&[
                "update-ref",
                "-d",
                &format!("refs/meow/snapshots/{}", snap.id),
            ]);
        }
    }
    idx.snapshots = kept;
    let _ = save_snapshots(&repo.git_dir, idx);
}

/// Restore working-tree files from a snapshot.
///
/// `also_remove_added` removes tracked files that exist in HEAD but were absent
/// from the snapshot. Used after a hard reset when undoing a `get`.
pub fn restore_snapshot(repo: &Repo, sha: &str, also_remove_added: bool) -> Result<()> {
    if also_remove_added {
        if let Some(raw) = repo
            .git
            .probe(&["diff", "--name-status", "-z", sha, "HEAD"])
        {
            let mut it = raw.split('\0');
            while let Some(status) = it.next() {
                let path = it.next().unwrap_or_default();
                if path.is_empty() {
                    continue;
                }
                // 'A' in snapshot..HEAD means HEAD added a file the snapshot
                // did not have, so restoring the snapshot should remove it.
                if status.starts_with('A') {
                    let full = repo.root.join(path);
                    if full.is_file() || full.is_symlink() {
                        let _ = fs::remove_file(&full);
                    }
                }
            }
        }
    }
    repo.git
        .check(&["restore", "--source", sha, "--worktree", "--", "."])
        .context("Couldn't restore the safety snapshot")?;
    let _ = repo.git.run(&["reset", "--quiet"]);
    Ok(())
}

pub fn remove_snapshot_ref(repo: &Repo, id: &str) {
    let _ = repo
        .git
        .run(&["update-ref", "-d", &format!("refs/meow/snapshots/{id}")]);
}

// ------------------------------------------------------------------------ trash

pub fn trash_dir(git_dir: &Path) -> PathBuf {
    meow_dir(git_dir).join("trash")
}

/// Move untracked files into Meow's trash. They are never deleted outright.
pub fn move_to_trash(repo: &Repo, paths: &[String], label: &str) -> Result<TrashMeta> {
    let id = new_id();
    let base = trash_dir(&repo.git_dir).join(&id);
    let files_dir = base.join("files");
    fs::create_dir_all(&files_dir)?;
    let mut moved = Vec::new();
    for rel in paths {
        let from = repo.root.join(rel);
        if !from.exists() {
            continue;
        }
        let to = files_dir.join(rel);
        if let Some(parent) = to.parent() {
            fs::create_dir_all(parent)?;
        }
        if fs::rename(&from, &to).is_err() {
            if from.is_dir() {
                copy_dir(&from, &to)?;
                fs::remove_dir_all(&from)?;
            } else {
                fs::copy(&from, &to)?;
                fs::remove_file(&from)?;
            }
        }
        moved.push(rel.clone());
        remove_empty_parents(&repo.root, &from);
    }
    let meta = TrashMeta {
        id,
        label: label.to_string(),
        at: Utc::now(),
        files: moved,
    };
    write_private(&base.join("meta.json"), serde_json::to_vec_pretty(&meta)?)?;
    restrict(&base);
    Ok(meta)
}

pub fn list_trash(git_dir: &Path) -> Vec<TrashMeta> {
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir(trash_dir(git_dir)) else {
        return out;
    };
    for entry in entries.flatten() {
        let meta = entry.path().join("meta.json");
        if let Ok(raw) = fs::read_to_string(meta) {
            if let Ok(m) = serde_json::from_str::<TrashMeta>(&raw) {
                out.push(m);
            }
        }
    }
    out.sort_by_key(|s| std::cmp::Reverse(s.at));
    out
}

pub fn restore_trash(repo: &Repo, meta: &TrashMeta, only: Option<&str>) -> Result<Vec<String>> {
    let base = trash_dir(&repo.git_dir).join(&meta.id).join("files");
    let mut restored = Vec::new();
    for rel in &meta.files {
        if let Some(wanted) = only {
            if rel != wanted {
                continue;
            }
        }
        let from = base.join(rel);
        if !from.exists() {
            continue;
        }
        let mut to = repo.root.join(rel);
        if to.exists() {
            let parent = to.parent().unwrap_or(&repo.root);
            let name = to.file_name().unwrap_or_default().to_string_lossy();
            to = parent.join(format!(".meow-recovered-{name}"));
        }
        if let Some(parent) = to.parent() {
            fs::create_dir_all(parent)?;
        }
        if fs::rename(&from, &to).is_err() {
            if from.is_dir() {
                copy_dir(&from, &to)?;
            } else {
                fs::copy(&from, &to)?;
            }
        }
        restored.push(rel.clone());
    }
    Ok(restored)
}

pub fn purge_trash(git_dir: &Path) -> Result<usize> {
    let dir = trash_dir(git_dir);
    let mut n = 0;
    if let Ok(entries) = fs::read_dir(&dir) {
        for entry in entries.flatten() {
            if entry.path().is_dir() {
                fs::remove_dir_all(entry.path())?;
                n += 1;
            }
        }
    }
    Ok(n)
}

fn copy_dir(from: &Path, to: &Path) -> Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.path().is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

fn remove_empty_parents(root: &Path, file: &Path) {
    let mut current = file.parent().map(Path::to_path_buf);
    while let Some(dir) = current {
        if dir == root || !dir.starts_with(root) {
            break;
        }
        if fs::read_dir(&dir)
            .map(|mut it| it.next().is_none())
            .unwrap_or(false)
        {
            let _ = fs::remove_dir(&dir);
            current = dir.parent().map(Path::to_path_buf);
        } else {
            break;
        }
    }
}
