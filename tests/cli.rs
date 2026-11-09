//! End-to-end tests: every command runs as a real binary against real Git.

use std::path::PathBuf;
use std::process::Command as StdCommand;

use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

struct Sandbox {
    _temp: TempDir,
    dir: PathBuf,
}

impl Sandbox {
    fn new() -> Sandbox {
        let temp = TempDir::new().expect("temp dir");
        let dir = temp.path().join("project");
        std::fs::create_dir_all(&dir).expect("project dir");
        let sandbox = Sandbox { _temp: temp, dir };
        sandbox.git(&["init", "-q", "-b", "main"]);
        sandbox.git(&["config", "user.name", "Ana Lima"]);
        sandbox.git(&["config", "user.email", "ana@example.com"]);
        sandbox
    }

    fn meow(&self) -> Command {
        let mut cmd = Command::cargo_bin("meow").expect("binary");
        cmd.current_dir(&self.dir);
        cmd.env("HOME", self._temp.path());
        cmd.env("XDG_CONFIG_HOME", self._temp.path().join("config"));
        cmd.env("XDG_STATE_HOME", self._temp.path().join("state"));
        cmd.env("NO_COLOR", "1");
        cmd
    }

    fn plain(&self) -> Command {
        let mut cmd = self.meow();
        cmd.arg("--plain");
        cmd
    }

    fn git(&self, args: &[&str]) -> String {
        let out = StdCommand::new("git")
            .args(args)
            .current_dir(&self.dir)
            .env("GIT_AUTHOR_NAME", "Ana Lima")
            .env("GIT_AUTHOR_EMAIL", "ana@example.com")
            .env("GIT_COMMITTER_NAME", "Ana Lima")
            .env("GIT_COMMITTER_EMAIL", "ana@example.com")
            .output()
            .expect("git runs");
        assert!(
            out.status.success(),
            "git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).to_string()
    }

    fn write(&self, path: &str, body: &str) {
        let full = self.dir.join(path);
        if let Some(parent) = full.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(full, body).unwrap();
    }

    fn read(&self, path: &str) -> String {
        std::fs::read_to_string(self.dir.join(path)).unwrap_or_default()
    }

    fn commit(&self, path: &str, body: &str, message: &str) {
        self.write(path, body);
        self.git(&["add", "-A"]);
        self.git(&["commit", "-q", "-m", message]);
    }

    fn log(&self) -> String {
        let out = StdCommand::new("git")
            .args(["log", "--format=%s"])
            .current_dir(&self.dir)
            .output()
            .expect("git runs");
        if !out.status.success() {
            return String::new();
        }
        String::from_utf8_lossy(&out.stdout).to_string()
    }

    fn status(&self) -> String {
        self.git(&["status", "--porcelain"])
    }
}

#[test]
fn home_outside_a_project_is_a_welcome_screen() {
    let temp = TempDir::new().unwrap();
    let mut cmd = Command::cargo_bin("meow").unwrap();
    cmd.current_dir(temp.path())
        .env("HOME", temp.path())
        .env("XDG_STATE_HOME", temp.path().join("state"))
        .env("NO_COLOR", "1");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("This folder isn't a project yet"))
        .stdout(predicate::str::contains("meow new"));
}

#[test]
fn new_starts_a_project_and_never_overwrites_files() {
    let temp = TempDir::new().unwrap();
    let dir = temp.path().join("portfolio");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("notes.txt"), "mine").unwrap();

    let global = temp.path().join("gitconfig");
    std::fs::write(
        &global,
        "[user]\n\tname = Ana Lima\n\temail = ana@example.com\n",
    )
    .unwrap();
    let mut cmd = Command::cargo_bin("meow").unwrap();
    cmd.current_dir(&dir)
        .env("HOME", temp.path())
        .env("GIT_CONFIG_GLOBAL", &global)
        .env("GIT_AUTHOR_NAME", "Ana Lima")
        .env("GIT_AUTHOR_EMAIL", "ana@example.com")
        .args(["new", "--plain"]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("project "));

    assert!(dir.join(".git").exists());
    assert_eq!(
        std::fs::read_to_string(dir.join("notes.txt")).unwrap(),
        "mine"
    );
    assert!(dir.join(".gitignore").exists());
    assert!(
        !dir.join("README.md").exists(),
        "README only on empty folders"
    );
    let branch = StdCommand::new("git")
        .args(["symbolic-ref", "--short", "HEAD"])
        .current_dir(&dir)
        .output()
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&branch.stdout).trim(), "main");
}

#[test]
fn save_commits_everything_and_reports_nothing_twice() {
    let s = Sandbox::new();
    s.write("a.txt", "one\n");
    s.plain().args(["save", "First save"]).assert().success();
    assert_eq!(s.log().trim(), "First save");

    s.plain()
        .arg("save")
        .assert()
        .success()
        .stdout(predicate::str::contains("nothing to save"));

    s.write("b.txt", "two\n");
    s.plain().args(["save", "Second save"]).assert().success();
    assert!(s.log().contains("Second save"));
}

#[test]
fn save_only_commits_the_named_paths() {
    let s = Sandbox::new();
    s.commit("keep.txt", "keep\n", "First");
    s.write("keep.txt", "changed\n");
    s.write("later.txt", "later\n");
    s.plain()
        .args(["save", "--only", "keep.txt", "Only keep"])
        .assert()
        .success();
    let status = s.status();
    assert!(
        status.contains("later.txt"),
        "later.txt stays unsaved: {status}"
    );
    assert!(!status.contains("keep.txt"), "keep.txt was saved: {status}");
}

#[test]
fn save_refuses_a_secret_in_plain_mode_unless_allowed() {
    let s = Sandbox::new();
    s.write(".env", "TOKEN=secret\n");
    s.plain().arg("save").assert().code(2);
    assert_eq!(s.log().trim(), "", "nothing was saved");

    s.plain()
        .args(["save", "--allow-secrets", "Add config"])
        .assert()
        .success();
    assert!(s.log().contains("Add config"));
}

#[test]
fn changes_shows_unsaved_work_and_saved_work() {
    let s = Sandbox::new();
    s.commit("a.txt", "one\n", "First");
    s.write("a.txt", "one\ntwo\n");
    s.plain()
        .arg("changes")
        .assert()
        .success()
        .stdout(predicate::str::contains("+two"))
        .stdout(predicate::str::contains("changes 1"));

    s.plain()
        .args(["changes", "--saved", "1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("saved "));
}

#[test]
fn history_lists_saves_and_restores_an_older_one() {
    let s = Sandbox::new();
    s.commit("a.txt", "one\n", "First");
    s.commit("a.txt", "two\n", "Second");

    s.plain()
        .arg("history")
        .assert()
        .success()
        .stdout(predicate::str::contains("Second"))
        .stdout(predicate::str::contains("First"));

    s.plain().args(["history", "2"]).assert().success();
    s.plain()
        .args(["history", "2", "--restore"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Brought save #2 back"));
    assert_eq!(s.read("a.txt"), "one\n");
    assert_eq!(
        s.log().lines().next().unwrap(),
        "Second",
        "history is untouched"
    );
}

#[test]
fn undo_and_redo_move_a_save_and_keep_the_files() {
    let s = Sandbox::new();
    s.write("a.txt", "one\n");
    s.plain().args(["save", "First"]).assert().success();
    s.plain().arg("undo").assert().success();
    assert_eq!(s.log().trim(), "", "save was taken back");
    assert_eq!(s.read("a.txt"), "one\n", "the work is still here");

    s.plain().arg("redo").assert().success();
    assert_eq!(s.log().trim(), "First");
}

#[test]
fn branch_create_switch_list_and_delete() {
    let s = Sandbox::new();
    s.commit("a.txt", "one\n", "First");
    s.plain().args(["branch", "add-gallery"]).assert().success();
    s.plain()
        .arg("branch")
        .assert()
        .success()
        .stdout(predicate::str::contains("add-gallery"));

    s.write("gallery.txt", "art\n");
    s.plain().args(["save", "Gallery"]).assert().success();
    s.plain().args(["branch", "main"]).assert().success();

    s.plain()
        .args(["branch", "delete", "add-gallery"])
        .assert()
        .code(2);
    s.plain()
        .args(["branch", "delete", "add-gallery", "--force"])
        .assert()
        .success();
    assert!(!s.git(&["branch"]).contains("add-gallery"));

    s.plain().arg("undo").assert().success();
    assert!(
        s.git(&["branch"]).contains("add-gallery"),
        "undo brings it back"
    );
}

#[test]
fn merge_fast_forwards_and_plain_mode_requires_yes() {
    let s = Sandbox::new();
    s.commit("a.txt", "one\n", "First");
    s.git(&["switch", "-q", "-c", "feature"]);
    s.commit("feature.txt", "feature\n", "Feature");
    s.git(&["switch", "-q", "main"]);

    s.plain().args(["merge", "feature"]).assert().code(2);
    s.plain()
        .args(["merge", "feature", "--yes"])
        .assert()
        .success()
        .stdout(predicate::str::contains("merged feature into main"));
    assert!(s.dir.join("feature.txt").exists());
}

#[test]
fn conflict_can_be_resolved_or_cancelled() {
    let s = Sandbox::new();
    s.commit("shared.txt", "line one\nshared\n", "First");
    s.git(&["switch", "-q", "-c", "feature"]);
    s.commit("shared.txt", "line one\nfeature\n", "Feature");
    s.git(&["switch", "-q", "main"]);
    s.commit("shared.txt", "line one\nmain\n", "Main");

    s.plain()
        .args(["merge", "feature", "--yes"])
        .assert()
        .code(1);
    assert!(s.status().contains("UU shared.txt"));

    s.plain()
        .arg("cancel")
        .assert()
        .success()
        .stdout(predicate::str::contains("cancelled"));
    assert_eq!(s.status().trim(), "");
    assert_eq!(s.read("shared.txt"), "line one\nmain\n");

    s.plain()
        .args(["merge", "feature", "--yes"])
        .assert()
        .code(1);
    s.plain().args(["resolve", "--theirs"]).assert().success();
    assert_eq!(s.read("shared.txt"), "line one\nfeature\n");
    assert!(s.log().contains("Merge branch 'feature'"));
    assert_eq!(s.status().trim(), "");
}

#[test]
fn resolve_both_keeps_each_side_once() {
    let s = Sandbox::new();
    s.commit("shared.txt", "line one\nshared\n", "First");
    s.git(&["switch", "-q", "-c", "feature"]);
    s.commit("shared.txt", "line one\nfeature\n", "Feature");
    s.git(&["switch", "-q", "main"]);
    s.commit("shared.txt", "line one\nmain\n", "Main");
    s.plain()
        .args(["merge", "feature", "--yes"])
        .assert()
        .code(1);
    s.plain().args(["resolve", "--both"]).assert().success();
    let text = s.read("shared.txt");
    assert_eq!(
        text.matches("line one").count(),
        1,
        "shared context once: {text}"
    );
    assert!(text.contains("main"));
    assert!(text.contains("feature"));
    assert!(text.contains("kept both versions"));
}

#[test]
fn discard_restores_tracked_files_and_trashes_untracked_ones() {
    let s = Sandbox::new();
    s.commit("a.txt", "one\n", "First");
    s.write("a.txt", "changed\n");
    s.write("draft.txt", "draft\n");

    s.plain().args(["discard", "a.txt"]).assert().success();
    assert_eq!(s.read("a.txt"), "one\n");

    s.plain().args(["discard", "draft.txt"]).assert().success();
    assert!(!s.dir.join("draft.txt").exists());
    assert_eq!(s.status().trim(), "");

    s.plain()
        .arg("recover")
        .assert()
        .success()
        .stdout(predicate::str::contains("trash"));
}

#[test]
fn ignore_updates_gitignore_and_stops_tracking() {
    let s = Sandbox::new();
    s.commit(".env", "SECRET=1\n", "Oops");
    s.plain().args(["ignore", ".env"]).assert().success();
    let ignore = s.read(".gitignore");
    assert!(ignore.contains(".env"));
    assert!(s.dir.join(".env").exists(), "file stays on disk");

    s.plain()
        .arg("ignore")
        .assert()
        .success()
        .stdout(predicate::str::contains("ignored .env"));
}

#[test]
fn send_and_get_use_a_local_home() {
    let s = Sandbox::new();
    s.commit("a.txt", "one\n", "First");
    let home = s._temp.path().join("home.git");
    let out = StdCommand::new("git")
        .args(["init", "-q", "--bare", home.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(out.status.success());
    s.git(&["remote", "add", "origin", home.to_str().unwrap()]);

    s.plain().arg("send").assert().success();

    let clone = s._temp.path().join("friend");
    let mut cmd = Command::cargo_bin("meow").unwrap();
    cmd.current_dir(s._temp.path())
        .env("HOME", s._temp.path())
        .args([
            "--plain",
            "get",
            home.to_str().unwrap(),
            "--folder",
            "friend",
        ]);
    cmd.assert().success();
    assert!(clone.join("a.txt").exists());

    // A change from the friend comes back with `meow get`.
    let friend = Sandbox {
        _temp: TempDir::new().unwrap(),
        dir: clone,
    };
    friend.git(&["config", "user.name", "Friend"]);
    friend.git(&["config", "user.email", "friend@example.com"]);
    friend.write("a.txt", "one\ntwo\n");
    friend.git(&["add", "-A"]);
    friend.git(&["commit", "-q", "-m", "Friend work"]);
    friend.git(&["push", "-q"]);

    s.plain()
        .arg("get")
        .assert()
        .success()
        .stdout(predicate::str::contains("got main 1 new save"));
    assert_eq!(s.read("a.txt"), "one\ntwo\n");
}

#[test]
fn doctor_and_version_run_anywhere() {
    let s = Sandbox::new();
    s.plain()
        .arg("doctor")
        .assert()
        .success()
        .stdout(predicate::str::contains("Git"))
        .stdout(predicate::str::contains("Name and email set"));

    s.plain()
        .arg("version")
        .assert()
        .success()
        .stdout(predicate::str::contains("meow 0.1.0"))
        .stdout(predicate::str::contains("git "));
}

#[test]
fn help_covers_every_command_and_the_git_mapping() {
    let temp = TempDir::new().unwrap();
    let mut cmd = Command::cargo_bin("meow").unwrap();
    cmd.current_dir(temp.path())
        .env("HOME", temp.path())
        .arg("help");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("meow save"))
        .stdout(predicate::str::contains("meow learn"));

    let mut cmd = Command::cargo_bin("meow").unwrap();
    cmd.current_dir(temp.path())
        .env("HOME", temp.path())
        .args(["help", "save"]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("git add -A"));

    let mut cmd = Command::cargo_bin("meow").unwrap();
    cmd.current_dir(temp.path())
        .env("HOME", temp.path())
        .args(["help", "--git"]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("meow discard"))
        .stdout(predicate::str::contains("git restore"));
}

#[test]
fn unknown_commands_fail_with_a_next_step() {
    let temp = TempDir::new().unwrap();
    let mut cmd = Command::cargo_bin("meow").unwrap();
    cmd.current_dir(temp.path())
        .env("HOME", temp.path())
        .arg("nonsense");
    cmd.assert()
        .code(2)
        .stdout(predicate::str::contains("meow help"));
}

#[test]
fn ascii_mode_has_no_emoji() {
    let s = Sandbox::new();
    s.commit("a.txt", "one\n", "First");
    let output = s
        .meow()
        .args(["--ascii", "history"])
        .output()
        .expect("runs");
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(!text.contains('✅'));
    assert!(output.status.success());
}

#[test]
fn history_can_filter_by_file_and_show_stats() {
    let s = Sandbox::new();
    s.commit("a.txt", "one\n", "First");
    s.commit("b.txt", "two\n", "Second");
    s.plain()
        .args(["history", "--file", "b.txt"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Second"))
        .stdout(predicate::str::contains("First").not());

    s.write("a.txt", "one\ntwo\n");
    s.plain()
        .args(["changes", "--stat"])
        .assert()
        .success()
        .stdout(predicate::str::contains("a.txt"));
}

#[test]
fn branch_delete_flag_works_and_undo_brings_it_back() {
    let s = Sandbox::new();
    s.commit("a.txt", "one\n", "First");
    s.git(&["branch", "scratch"]);
    s.plain()
        .args(["branch", "--delete", "scratch"])
        .assert()
        .success();
    assert!(!s.git(&["branch"]).contains("scratch"));

    s.plain().arg("undo").assert().success();
    assert!(s.git(&["branch"]).contains("scratch"));
}

#[test]
fn resolve_ours_keeps_the_current_branch() {
    let s = Sandbox::new();
    s.commit("shared.txt", "line one\nshared\n", "First");
    s.git(&["switch", "-q", "-c", "feature"]);
    s.commit("shared.txt", "line one\nfeature\n", "Feature");
    s.git(&["switch", "-q", "main"]);
    s.commit("shared.txt", "line one\nmain\n", "Main");
    s.plain()
        .args(["merge", "feature", "--yes"])
        .assert()
        .code(1);
    s.plain().args(["resolve", "--ours"]).assert().success();
    assert_eq!(s.read("shared.txt"), "line one\nmain\n");
    assert_eq!(s.status().trim(), "");
}

#[test]
fn doctor_report_and_learn_work_without_a_terminal() {
    let s = Sandbox::new();
    s.commit("a.txt", "one\n", "First");
    s.plain()
        .args(["doctor", "--report"])
        .assert()
        .success()
        .stdout(predicate::str::contains("report "));

    let output = s.plain().arg("learn").output().expect("runs");
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    assert!(text.contains("learn chapters 8"));
}
