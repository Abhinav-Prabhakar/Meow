//! A read-mostly model of the repository: state, branches, history, remotes.

use std::path::{Path, PathBuf};

use anyhow::Result;
use chrono::{DateTime, FixedOffset};

use crate::git::Git;
use crate::ui::Ui;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChangeKind {
    Added,
    Modified,
    Deleted,
    Renamed,
    TypeChange,
    Untracked,
    Conflicted,
}

#[derive(Debug, Clone)]
pub struct FileChange {
    pub path: String,
    pub kind: ChangeKind,
}

impl FileChange {
    pub fn is_untracked(&self) -> bool {
        self.kind == ChangeKind::Untracked
    }

    pub fn code(&self) -> &'static str {
        match self.kind {
            ChangeKind::Added => "A",
            ChangeKind::Modified => "M",
            ChangeKind::Deleted => "D",
            ChangeKind::Renamed => "R",
            ChangeKind::TypeChange => "T",
            ChangeKind::Untracked => "?",
            ChangeKind::Conflicted => "U",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operation {
    Merge,
    Rebase,
    CherryPick,
    Revert,
    Bisect,
}

impl Operation {
    pub fn label(self) -> &'static str {
        match self {
            Operation::Merge => "combine",
            Operation::Rebase => "rebase",
            Operation::CherryPick => "cherry-pick",
            Operation::Revert => "revert",
            Operation::Bisect => "bisect",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Status {
    pub branch: Option<String>,
    pub detached: bool,
    pub upstream: Option<String>,
    pub ahead: usize,
    pub behind: usize,
    pub files: Vec<FileChange>,
    pub operation: Option<Operation>,
    pub conflicts: Vec<String>,
}

impl Status {
    pub fn dirty(&self) -> bool {
        !self.files.is_empty()
    }

    pub fn conflicted(&self) -> bool {
        !self.conflicts.is_empty()
    }
}

#[derive(Debug, Clone)]
pub struct Branch {
    pub name: String,
    pub sha: String,
    pub when: Option<DateTime<FixedOffset>>,
    pub merged: bool,
}

#[derive(Debug, Clone)]
pub struct Commit {
    pub sha: String,
    pub short: String,
    pub author: String,
    pub when: DateTime<FixedOffset>,
    pub subject: String,
}

#[derive(Debug, Clone)]
pub struct HeadInfo {
    pub subject: String,
    pub author: String,
    pub when: DateTime<FixedOffset>,
}

#[derive(Debug, Clone)]
pub struct Remote {
    pub name: String,
    pub url: String,
}

pub struct Repo {
    pub root: PathBuf,
    pub git_dir: PathBuf,
    pub git: Git,
}

impl Repo {
    /// Find the project containing `cwd`. Returns `None` when outside a project.
    pub fn discover(cwd: &Path, ui: &Ui) -> Result<Option<Repo>> {
        let git = Git::new(cwd, ui);
        let Some(top) = git.probe(&["rev-parse", "--show-toplevel"]) else {
            return Ok(None);
        };
        if top.is_empty() {
            return Ok(None);
        }
        let root = PathBuf::from(&top);
        let bare = git
            .probe(&["rev-parse", "--is-bare-repository"])
            .map(|s| s == "true")
            .unwrap_or(false);
        if bare {
            return Ok(None);
        }
        let git_dir = git
            .probe(&["rev-parse", "--git-common-dir"])
            .map(PathBuf::from)
            .map(|p| if p.is_absolute() { p } else { root.join(p) })
            .unwrap_or_else(|| root.join(".git"));
        let git = Git::new(&root, ui);
        Ok(Some(Repo { root, git_dir, git }))
    }

    pub fn status(&self) -> Result<Status> {
        let out = self
            .git
            .check(&["status", "--porcelain=v2", "--branch", "-z"])?;
        let mut status = parse_status(&out.stdout);
        status.operation = self.detect_operation();
        Ok(status)
    }

    /// Which multi-step Git operation, if any, is in progress.
    fn detect_operation(&self) -> Option<Operation> {
        let exists = |name: &str| -> bool {
            self.git
                .probe(&["rev-parse", "--git-path", name])
                .map(|p| Path::new(&p).exists())
                .unwrap_or(false)
        };
        if exists("rebase-merge") || exists("rebase-apply") {
            return Some(Operation::Rebase);
        }
        if exists("MERGE_HEAD") {
            return Some(Operation::Merge);
        }
        if exists("CHERRY_PICK_HEAD") {
            return Some(Operation::CherryPick);
        }
        if exists("REVERT_HEAD") {
            return Some(Operation::Revert);
        }
        if exists("BISECT_LOG") {
            return Some(Operation::Bisect);
        }
        None
    }

    pub fn head(&self) -> Option<String> {
        self.git.head()
    }

    pub fn has_commits(&self) -> bool {
        self.git.has_commits()
    }

    pub fn branch(&self) -> Option<String> {
        self.git.branch_name()
    }

    pub fn head_info(&self) -> Result<HeadInfo> {
        let raw = self
            .git
            .out(&["log", "-1", "--pretty=format:%H%x00%s%x00%an%x00%aI"])?;
        let mut parts = raw.split('\0');
        let _sha = parts.next().unwrap_or_default().to_string();
        let subject = parts.next().unwrap_or_default().to_string();
        let author = parts.next().unwrap_or_default().to_string();
        let when = parts
            .next()
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .unwrap_or_else(|| {
                DateTime::parse_from_rfc3339("1970-01-01T00:00:00Z").expect("epoch")
            });
        Ok(HeadInfo {
            subject,
            author,
            when,
        })
    }

    pub fn commit_count(&self) -> usize {
        self.git
            .probe(&["rev-list", "--count", "HEAD"])
            .and_then(|s| s.parse().ok())
            .unwrap_or(0)
    }

    pub fn remotes(&self) -> Vec<Remote> {
        let Some(raw) = self.git.probe(&["remote"]) else {
            return Vec::new();
        };
        raw.lines()
            .filter(|l| !l.trim().is_empty())
            .map(|name| Remote {
                name: name.trim().to_string(),
                url: self
                    .git
                    .probe(&["remote", "get-url", name.trim()])
                    .unwrap_or_default(),
            })
            .collect()
    }

    /// The chosen home: `origin` when present, otherwise the only remote.
    pub fn home_remote(&self) -> Option<Remote> {
        let remotes = self.remotes();
        if remotes.is_empty() {
            return None;
        }
        if let Some(origin) = remotes.iter().find(|r| r.name == "origin") {
            return Some(origin.clone());
        }
        if remotes.len() == 1 {
            return Some(remotes[0].clone());
        }
        remotes.into_iter().find(|r| r.name == "upstream")
    }

    pub fn home_label(&self) -> Option<String> {
        self.home_remote().map(|r| pretty_remote(&r.url))
    }

    pub fn branches(&self) -> Result<Vec<Branch>> {
        let current = self.branch().unwrap_or_default();
        let merged: Vec<String> = self
            .git
            .probe(&["branch", "--format=%(refname:short)", "--merged", &current])
            .map(|s| s.lines().map(|l| l.trim().to_string()).collect())
            .unwrap_or_default();
        let raw = self.git.out(&[
            "for-each-ref",
            "--format=%(refname:short)%00%(objectname)%00%(committerdate:iso-strict)",
            "refs/heads",
        ])?;
        let mut out = Vec::new();
        for line in raw.lines() {
            let mut f = line.split('\0');
            let name = f.next().unwrap_or_default().to_string();
            if name.is_empty() {
                continue;
            }
            let sha = f.next().unwrap_or_default().to_string();
            let when = f.next().and_then(|s| DateTime::parse_from_rfc3339(s).ok());
            out.push(Branch {
                merged: merged.iter().any(|m| m == &name),
                name,
                sha,
                when,
            });
        }
        Ok(out)
    }

    pub fn commits(&self, limit: usize, all: bool, file: Option<&str>) -> Result<Vec<Commit>> {
        let n = format!("-{limit}");
        let mut args = vec![
            "log",
            &n,
            "--date-order",
            "--pretty=format:%H%x00%h%x00%an%x00%aI%x00%s",
        ];
        if all {
            args.insert(1, "--all");
        }
        if let Some(path) = file {
            args.push("--");
            args.push(path);
        }
        let raw = self.git.check(&args)?;
        Ok(raw
            .stdout
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|line| {
                let mut f = line.split('\0');
                let sha = f.next().unwrap_or_default().to_string();
                let short = f.next().unwrap_or_default().to_string();
                let author = f.next().unwrap_or_default().to_string();
                let when = f
                    .next()
                    .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                    .unwrap_or_else(|| {
                        DateTime::parse_from_rfc3339("1970-01-01T00:00:00Z").expect("epoch")
                    });
                let subject = f.next().unwrap_or_default().to_string();
                Commit {
                    sha,
                    short,
                    author,
                    when,
                    subject,
                }
            })
            .collect())
    }

    pub fn commits_between(&self, from: &str, to: &str) -> Vec<String> {
        self.git
            .probe(&["log", "--oneline", &format!("{from}..{to}")])
            .map(|s| s.lines().map(str::to_string).collect())
            .unwrap_or_default()
    }

    pub fn diff_stat(&self) -> DiffStat {
        let mut stat = DiffStat::default();
        if self.has_commits() {
            if let Some(raw) = self.git.probe(&["diff", "HEAD", "--numstat", "-z"]) {
                for rec in raw.split('\0').filter(|s| !s.is_empty()) {
                    let mut f = rec.split('\t');
                    let add = f.next().unwrap_or("0");
                    let del = f.next().unwrap_or("0");
                    let _path = f.next().unwrap_or("");
                    stat.files += 1;
                    stat.added += add.parse::<usize>().unwrap_or(0);
                    stat.deleted += del.parse::<usize>().unwrap_or(0);
                }
            }
        }
        if let Some(raw) = self
            .git
            .probe(&["ls-files", "--others", "--exclude-standard", "-z"])
        {
            for rel in raw.split('\0').filter(|s| !s.is_empty()) {
                stat.files += 1;
                stat.added += count_lines(&self.root.join(rel));
            }
        }
        stat
    }

    pub fn commit_stat(&self, sha: &str) -> DiffStat {
        let mut stat = DiffStat::default();
        if let Some(raw) = self
            .git
            .probe(&["show", "--numstat", "--format=", "-z", sha])
        {
            for rec in raw.split('\0').filter(|s| !s.is_empty()) {
                let mut f = rec.split('\t');
                let add = f.next().unwrap_or("0");
                let del = f.next().unwrap_or("0");
                stat.files += 1;
                stat.added += add.parse::<usize>().unwrap_or(0);
                stat.deleted += del.parse::<usize>().unwrap_or(0);
            }
        }
        stat
    }

    pub fn is_ancestor(&self, ancestor: &str, descendant: &str) -> bool {
        self.git
            .probe_bool(&["merge-base", "--is-ancestor", ancestor, descendant])
    }

    pub fn identity(&self) -> Option<(String, String)> {
        let name = self.git.probe(&["config", "user.name"])?;
        let email = self.git.probe(&["config", "user.email"])?;
        if name.trim().is_empty() || email.trim().is_empty() {
            return None;
        }
        Some((name.trim().to_string(), email.trim().to_string()))
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct DiffStat {
    pub files: usize,
    pub added: usize,
    pub deleted: usize,
}

fn count_lines(path: &Path) -> usize {
    let Ok(meta) = std::fs::metadata(path) else {
        return 0;
    };
    if !meta.is_file() || meta.len() > 2_000_000 {
        return 0;
    }
    std::fs::read_to_string(path)
        .map(|s| s.lines().count())
        .unwrap_or(0)
}

pub fn parse_status(raw: &str) -> Status {
    let mut status = Status {
        branch: None,
        detached: false,
        upstream: None,
        ahead: 0,
        behind: 0,
        files: Vec::new(),
        operation: None,
        conflicts: Vec::new(),
    };
    let mut records = raw.split('\0').peekable();
    while let Some(rec) = records.next() {
        if rec.is_empty() {
            continue;
        }
        if let Some(header) = rec.strip_prefix("# ") {
            if let Some(v) = header.strip_prefix("branch.head ") {
                if v == "(detached)" {
                    status.detached = true;
                } else {
                    status.branch = Some(v.to_string());
                }
            } else if let Some(v) = header.strip_prefix("branch.upstream ") {
                status.upstream = Some(v.to_string());
            } else if let Some(v) = header.strip_prefix("branch.ab ") {
                let mut parts = v.split_whitespace();
                status.ahead = parts
                    .next()
                    .and_then(|s| s.trim_start_matches('+').parse().ok())
                    .unwrap_or(0);
                status.behind = parts
                    .next()
                    .and_then(|s| s.trim_start_matches('-').parse().ok())
                    .unwrap_or(0);
            }
            continue;
        }
        let kind = rec.chars().next().unwrap_or('?');
        match kind {
            '1' => {
                if let Some(fc) = parse_v2_change(rec) {
                    status.files.push(fc);
                }
            }
            '2' => {
                // Rename records are followed by the original path.
                let _original = records.next();
                if let Some(fc) = parse_v2_change(rec) {
                    status.files.push(fc);
                }
            }
            'u' => {
                if let Some(fc) = parse_v2_change(rec) {
                    status.conflicts.push(fc.path.clone());
                    status.files.push(fc);
                }
            }
            '?' => {
                let path = rec[2..].to_string();
                status.files.push(FileChange {
                    path,
                    kind: ChangeKind::Untracked,
                });
            }
            _ => {}
        }
    }
    status
}

fn parse_v2_change(rec: &str) -> Option<FileChange> {
    // 1 <XY> <sub> <mH> <mI> <mW> <hH> <hI> <path>
    // 2 <XY> <sub> <mH> <mI> <mW> <hH> <hI> <X><score> <path>
    // u <XY> <sub> <m1> <m2> <m3> <mW> <h1> <h2> <h3> <path>
    let fields: Vec<&str> = rec.splitn(10, ' ').collect();
    if fields.len() < 3 {
        return None;
    }
    let xy = fields.get(1).copied().unwrap_or("..");
    let mut chars = xy.chars();
    let x = chars.next().unwrap_or('.');
    let y = chars.next().unwrap_or('.');
    let path = if fields[0] == "u" {
        rec.splitn(11, ' ').nth(10).unwrap_or("").to_string()
    } else if fields[0] == "2" {
        rec.splitn(10, ' ').nth(9).unwrap_or("").to_string()
    } else {
        fields.get(8).copied().unwrap_or("").to_string()
    };
    if path.is_empty() {
        return None;
    }
    let kind = if fields[0] == "u" || x == 'U' || y == 'U' {
        ChangeKind::Conflicted
    } else if x == 'R' || y == 'R' {
        ChangeKind::Renamed
    } else if x == 'D' || y == 'D' {
        ChangeKind::Deleted
    } else if x == 'A' || y == 'A' {
        ChangeKind::Added
    } else if x == 'T' || y == 'T' {
        ChangeKind::TypeChange
    } else {
        ChangeKind::Modified
    };
    Some(FileChange { path, kind })
}

/// Turn a remote URL into the calm `host/path` label used on screen.
pub fn pretty_remote(url: &str) -> String {
    let mut s = url.trim().to_string();
    if let Some(rest) = s.strip_prefix("git@") {
        s = rest.replacen(':', "/", 1);
    } else if let Some(rest) = s.strip_prefix("ssh://git@") {
        s = rest.to_string();
    } else if let Some(rest) = s.strip_prefix("ssh://") {
        s = rest.to_string();
    } else if let Some(rest) = s.strip_prefix("https://") {
        s = rest.to_string();
    } else if let Some(rest) = s.strip_prefix("http://") {
        s = rest.to_string();
    } else if let Some(rest) = s.strip_prefix("file://") {
        s = rest.to_string();
    }
    if let Some(rest) = s.strip_prefix("git@") {
        s = rest.replacen(':', "/", 1);
    }
    if let Some(stripped) = s.strip_suffix(".git") {
        s = stripped.to_string();
    }
    if let Some((user, rest)) = s.split_once('@') {
        if !rest.is_empty() {
            s = rest.to_string();
        } else {
            s = user.to_string();
        }
    }
    // Local paths can pick up `./` and trailing slashes; keep them readable.
    while s.contains("/./") {
        s = s.replace("/./", "/");
    }
    if s.ends_with('/') && s.len() > 1 {
        s.pop();
    }
    s
}

/// Normalize a link or `owner/repo` shorthand into a clone URL.
pub fn normalize_link(input: &str) -> String {
    let s = input.trim();
    if s.starts_with("http://")
        || s.starts_with("https://")
        || s.starts_with("ssh://")
        || s.starts_with("git@")
        || s.starts_with("file://")
        || s.starts_with('/')
        || s.starts_with('.')
    {
        return s.to_string();
    }
    let parts: Vec<&str> = s.split('/').collect();
    if parts.len() == 2 && !parts[0].is_empty() && !parts[1].is_empty() {
        return format!("https://github.com/{}/{}.git", parts[0], parts[1]);
    }
    s.to_string()
}

/// A folder name that is safe to create for a clone.
pub fn folder_name_from_link(link: &str) -> String {
    let mut s = link.trim_end_matches('/').to_string();
    if let Some(stripped) = s.strip_suffix(".git") {
        s = stripped.to_string();
    }
    let tail = s
        .rsplit(['/', ':'])
        .find(|p| !p.is_empty())
        .unwrap_or("project");
    let cleaned: String = tail
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' || c == '.' {
                c
            } else {
                '-'
            }
        })
        .collect();
    if cleaned.is_empty() {
        "project".to_string()
    } else {
        cleaned
    }
}
