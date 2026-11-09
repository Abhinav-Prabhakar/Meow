# The meow guide

This is the long version. If you only want to get going, read [START.md](START.md) instead.

- [The mental model](#the-mental-model)
- [The everyday loop](#the-everyday-loop)
- [Command reference](#command-reference)
- [Conflicts, step by step](#conflicts-step-by-step)
- [The safety net](#the-safety-net)
- [Sharing and hosting](#sharing-and-hosting)
- [Branches and combining](#branches-and-combining)
- [Secrets, large files, and ignoring](#secrets-large-files-and-ignoring)
- [Plain mode and scripting](#plain-mode-and-scripting)
- [Configuration](#configuration)
- [Git interoperability](#git-interoperability)
- [Troubleshooting](#troubleshooting)
- [Windows notes](#windows-notes)
- [When you outgrow meow](#when-you-outgrow-meow)

---

## The mental model

Meow has five nouns and seven verbs. That is the whole vocabulary.

| Word | What it means |
|---|---|
| **Project** | A folder Meow is tracking. A normal Git repository. |
| **Save** | A checkpoint of your work. A normal Git commit. |
| **Home** | The online copy of your project. A Git remote. |
| **Branch** | A parallel version of your project. |
| **Snapshot** | A private local safety copy Meow makes before risky actions. |

| Verb | What it does |
|---|---|
| `new` | Start tracking a folder. |
| `save` | Checkpoint everything that changed. |
| `send` | Put saves online. |
| `get` | Bring new saves down, or copy a project here. |
| `changes` | Show unsaved work. |
| `history` | Show past saves. |
| `undo` | Take back the last action. |

Everything else — branches, combining, conflicts, recovery — exists to support those five.

### What Meow deliberately hides

- **The staging area.** You never stage files. `meow save` saves everything, or exactly the paths you name with `--only`.
- **Detached HEAD.** Switching to an old save is a read-only preview of the past. Meow puts you back on a branch before you can get lost.
- **Force pushes.** There is no `--force`. If a send is rejected, Meow gets the other person's work first, then sends yours.
- **Commit hashes and object IDs.** They exist, but only appear when you ask (`--long`, `--verbose`, `--explain`).

### What Meow keeps

Real commits, real branches, real remotes, real hooks, real signing, and real credentials. Your project is a normal Git project. The proof is that `git log` works on it.

---

## The everyday loop

Most days look like this:

```sh
meow                      # what is going on?
meow save "Fix the header"  # checkpoint
meow send                 # share it
meow get                  # pick up teammates' work
```

That is it. The home screen is designed to tell you the next step so you do not have to remember this list.

### A day in full

```text
  meow · ~/Projects/portfolio

  You're on main.
  ☁️  github.com/ana/portfolio

  ● 2 unsaved changes                    +48 -12
  ↑ 1 save not sent yet
  ↓ 3 new saves from your team

  Next
    meow save "Add project photos"       save your changes
    meow get                             get the latest
```

Read it top to bottom:

1. **Where you are** — the project and branch.
2. **What is true** — unsaved changes, saves waiting to go out, saves waiting to come in.
3. **What to do** — at most two suggestions, in priority order.

There is no hidden state. If the screen does not mention something, it is not happening.

---

## Command reference

### `meow` — where am I?

```sh
meow
meow --plain
```

The home screen. It adapts to the state of the project: clean, dirty, mid-combine, detached, outside a project, or not yet a project.

Outside a project it becomes a welcome screen with three exits: start a project, copy one, or practice.

### `meow new [name]` — start a project

```sh
meow new portfolio        # create ./portfolio and track it
meow new                  # track the current folder
```

What happens:

1. The folder is created if needed.
2. Meow checks whether Git and your name/email are configured. It only asks when a name is actually needed — at the first save — and explains why.
3. It starts the project on `main` (or your configured default branch).
4. It detects the kind of project (Node, Rust, Python, and so on) and proposes a `.gitignore` before the first save. This is the single highest-value moment to keep `node_modules` out of history forever.
5. If the folder is empty, it writes a short starter `README.md`. If files already exist, it **never overwrites anything**.
6. It ends on the home screen with the next step.

### `meow save ["note"]` — checkpoint your work

```sh
meow save
meow save "Add the gallery layout"
meow save --only src/app.js "Just the app"
```

A save includes every changed file, new file, and deleted file. There is no staging step.

- If you omit the note, Meow writes a short one for you.
- If nothing changed, it says so and does nothing.
- `--only <path>` saves just those paths, leaving everything else unsaved. This is the advanced escape hatch, not the normal flow.
- If something looks like a secret (`.env`, a private key, a token) or a very large file, Meow stops and explains. You can ignore the file, or pass `--allow-secrets` / `--allow-large`.

A save is **local**. `meow send` is what puts it online.

If a pre-commit hook fails, Meow says "A project check blocked your save" and shows the hook's output in an indented block. It does not offer a bypass.

### `meow send` — put your saves online

```sh
meow send
```

- If the project already has a home, Meow sends your saves there.
- If it does not, Meow uses the GitHub CLI (`gh`) to create a repository, set `origin`, and send. If `gh` is missing or not signed in, it asks you to paste a repository link instead.
- If someone else sent first, Meow gets their work, then sends yours. You are never asked to force anything.
- If the home protects the branch, Meow explains the situation instead of dumping an HTTP error.

Sent history is permanent. `meow undo` will not rewrite it; it offers a new save that reverses the old one.

### `meow get [link]` — get new work, or copy a project

```sh
meow get                                  # latest for this project
meow get github.com/ana/portfolio         # copy it into ./portfolio
meow get github.com/ana/portfolio --folder site
```

**With no link:** fetch and bring in new saves for the current branch.

- If you have unsaved changes, Meow sets them aside in a stash, gets the latest, then puts them back.
- If the incoming work conflicts with your unsaved work, you get conflict markers and a clear next step. Nothing is lost.
- If the home has no saves yet, Meow says so and offers `meow send`.

**With a link:** copy the project into a new folder. Meow understands full URLs, SSH links, `file://`, local paths, and the `owner/repo` shorthand.

### `meow changes [path]` — see what changed

```sh
meow changes
meow changes src/
meow changes --stat
meow changes --saved 3
```

- No arguments: every unsaved change, including new files (which plain `git diff` omits).
- `path`: limit to one file or folder.
- `--stat`: summary only.
- `--saved N`: show what the Nth most recent save changed, instead of unsaved work.

Binary files get a size summary instead of a wall of noise.

### `meow history [n]` — see past saves

```sh
meow history
meow history 3
meow history --all
meow history --file src/app.js
meow history --long
meow history 3 --restore
```

- No arguments: the last ten saves on this branch, grouped by day.
- `n`: details for the Nth most recent save.
- `--all`: include other branches.
- `--file <path>`: only saves that touched that path.
- `--long`: exact timestamps and short IDs.
- `--restore`: bring the files from that save back as **unsaved changes**. History is not rewritten.

Restoring an old version is deliberately non-destructive: the old contents become unsaved work, and you choose whether to save them.

### `meow undo [n]` / `meow redo` — take back an action

```sh
meow undo
meow undo 2
meow undo --list
meow redo
```

`undo` works from Meow's own journal, so it can reverse:

- a save (the commit is removed; your files stay as they are);
- a branch switch, creation, or deletion;
- a combine;
- a get;
- a discard, an ignore, or a restore.

Rules:

- For a save, if the branch moved since the action, Meow refuses rather than guessing.
- A **sent** save is never rewritten. Undoing it offers a new save that reverses it.
- Undoing a save keeps your files. It only removes the checkpoint.
- Sending itself cannot be undone; the work is already online.
- `redo` puts back the last thing you undid, if the project has not changed since.

Actions done outside Meow are not in the journal.

### `meow branch [name]` — list, start, or switch

```sh
meow branch                     # list branches
meow branch add-gallery         # start it, or switch to it if it exists
meow branch delete add-gallery  # delete it
meow branch --delete add-gallery
```

One command for "I want to work on add-gallery". Whether the branch already exists is an implementation detail.

- Switching with unsaved work asks what to do first.
- A name close to an existing branch gets a "did you mean" prompt.
- Deleting an uncombined branch needs an explicit confirmation and keeps a hidden copy that `meow recover` can bring back.
- A switch always takes a safety snapshot first.

### `meow merge <name>` — combine another branch into yours

```sh
meow merge add-gallery
meow merge add-gallery --yes
```

`meow merge` shows a preview before changing anything:

- how many saves are coming in;
- how many files, and how many lines were added or removed;
- the last save and who wrote it;
- whether the combine is a simple fast-forward or will create a new save;
- which files will need your help, predicted without touching the working tree.

Then you choose: combine, see the exact diff, or cancel.

If the working tree is dirty, Meow offers to save first. Conflicts are not treated as an error — they are a normal step with a clear handoff to `meow resolve`.

### `meow resolve` — fix conflicts, one block at a time

```sh
meow resolve
meow resolve --list
meow resolve --ours
meow resolve --theirs
meow resolve --both
```

No arguments: walk through every conflicted file. For each conflicting block you can:

- **Keep mine** — keep your branch's version;
- **Take theirs** — take the other branch's version;
- **Keep both** — include both, separated by a comment appropriate to the file type;
- **Open in my editor** — edit it yourself;
- **Decide later** — leave it for now.

`--list` shows which files still need help.

`--ours`, `--theirs`, and `--both` resolve every conflict one way at once. `--both` works block by block, so shared context above and below a conflict is not duplicated. The separator depends on the file extension (`//`, `#`, `<!-- -->`, `--`).

Binary files and files deleted on one side get a simple choice instead of a broken text view.

When every conflict is resolved, Meow offers to finish the combine and create one save with both sides.

### `meow cancel` — stop a combine

```sh
meow cancel
```

Returns the project to exactly where it was before the combine started. Both versions are kept. Nothing is deleted.

If the project is in the middle of a different multi-step Git operation, Meow says so and names the Git command instead of guessing.

### `meow discard [path...]` — throw away unsaved changes

```sh
meow discard
meow discard src/experiment.js
meow discard --all
```

- Tracked files are restored to their state at the last save.
- Untracked files are **moved to Meow's trash**, not deleted.
- A safety snapshot is taken first.
- Discarding mid-combine is refused: resolve or cancel instead.

### `meow ignore [path]` — leave something out

```sh
meow ignore node_modules
meow ignore .env
meow ignore --list
meow ignore
```

- Adds the path to `.gitignore`.
- If the path is already tracked, Meow stops tracking it but leaves it on disk.
- If it is already in history, Meow says so plainly and warns you to change any secret. It will **not** rewrite history on its own.
- With no path, Meow suggests things worth ignoring based on what is in the folder.
- Meow refuses to ignore `.git`, and double-checks patterns that would match everything or your source files.

### `meow recover` — find work you thought you lost

```sh
meow recover
meow recover --purge
```

Lists safety snapshots and discarded files, newest first, with what they contain and when. Choose one to restore:

- Snapshots restore selected files from a private copy. Your current version is snapshotted first, so recovery itself is reversible.
- Trash restores discarded untracked files. If a file exists again, the recovered copy gets a `.meow-recovered-<name>` name.
- `--purge` permanently empties the trash and removes snapshots. It asks first.

Everything here is local. Nothing is uploaded.

### `meow doctor` — check your setup

```sh
meow doctor
meow doctor --fix
meow doctor --report
```

Checks:

- Git is installed and at least 2.30;
- name and email are configured;
- GitHub CLI is installed and signed in (optional);
- a credential helper is configured;
- the current project's health, including half-finished operations;
- the online home, `.gitignore`, and very large files in history;
- whether Meow's undo journal is readable;
- safety copies waiting to be recovered;
- disk space.

`--fix` handles safe, common fixes with consent, starting with a missing name and email.

`--report` writes a redacted text file (no tokens, home directory shortened to `~`) that you can choose to share.

### `meow help` — learn the tool

```sh
meow help
meow help save
meow help --git
meow help --git save
```

Every command page has the same shape: what it does, examples, the Git underneath, and common mistakes. `--git` is the full translation table.

### `meow learn` — practice safely

```sh
meow learn
```

An interactive, ten-minute session in a throwaway folder. It walks through saving, seeing changes, undoing, branching, combining, fixing a conflict, sharing (with a simulated local home), and recovering. No real project, account, or network is touched. Quitting is always allowed.

### `meow version`

```sh
meow version
meow version --verbose
```

Shows Meow and Git versions. `--verbose` adds config and state paths, the current folder, and your shell. Meow has no self-updater; upgrade through the package manager you used.

---

## Conflicts, step by step

A conflict means two people changed the same part of the same file. Git cannot know which version is right, so it asks you. Nothing is lost.

### What you see

```text
  Combine add-gallery into main
  ──────────────────────────────────────────────────────────

  ⚠️ This will need your help in 1 file.

  Combining 1 save…

  ❌ 1 file needs your help

  You and someone else changed the same parts. Nothing is lost.

  Next
    meow resolve  fix them one at a time
    meow cancel   stop and go back
```

### Walk through it

```sh
meow resolve
```

For each block, Meow shows both sides side by side and asks what this part should say. You can choose one side, keep both, open your editor, or defer.

Some practical advice:

- **Keep mine** when your change is the one you meant to make.
- **Take theirs** when their change supersedes yours.
- **Keep both** when the two changes are independent (for example, two new functions). Check that the result still compiles or runs.
- **Open in my editor** for anything structural.

### Finish

When nothing is left, Meow offers to finish the combine. That creates one save containing both sides.

### Change your mind

```sh
meow cancel
```

You are back exactly where you started, with both branches intact.

---

## The safety net

This is the feature that makes the rest of the tool safe to use. It is not a command; it is a property of every command.

### What is protected

Before any operation that could change or remove local work — `get`, `merge`, a branch switch, `discard`, `undo`, `cancel` — Meow takes a snapshot of the working tree, including:

- modified tracked files;
- deleted tracked files;
- untracked files that are not ignored;
- the current branch and position;
- the state of any in-progress combine.

Snapshots are stored in the repository's private local storage. They are deduplicated and compressed, so unchanged files cost almost nothing. They are never uploaded.

### Retention

- The last **20** snapshots are kept, plus anything younger than **30 days**.
- Pruning happens automatically. `meow doctor` reports how many copies exist; the daily flow does not nag.
- `meow recover --purge` removes them immediately, after asking.

### Trash

Files removed by `meow discard` are moved into a private trash, with their paths. Recovery puts them back. If a path is occupied, the recovered copy is renamed rather than overwriting anything.

### What is not recoverable

- **Sent history.** Once a save is online, Meow will not rewrite it. Undo offers a reversing save.
- **Work done outside Meow.** The undo journal only knows Meow's actions. Snapshots still help after `get`, `merge`, and similar operations, but a `git reset --hard` run in another terminal is outside the net.
- **Files already purged.** `--purge` is permanent.

---

## Sharing and hosting

Meow is host-agnostic: any Git remote works. The convenience path is GitHub.

### With the GitHub CLI

If `gh` is installed and signed in:

```sh
meow send
```

Meow creates the repository, adds `origin`, and sends the current branch. It uses `--public` or `--private` based on your choice.

### Without the GitHub CLI

Meow asks you to paste a repository URL (for example, one you created in a browser). It adds it as `origin` and sends.

You can also add a remote yourself with Git; Meow will use it.

### Teams

- `meow get` brings in new saves.
- `meow send` sends yours.
- If both happened, Meow combines them instead of rejecting one side.
- Protected branches are explained in plain words, with the next step.

---

## Branches and combining

A branch is a parallel version. Use one when you want to try something without disturbing the main line.

```sh
meow branch add-gallery     # start or switch
meow branch                 # list
meow branch main            # switch back
meow merge add-gallery      # combine it into main
meow branch delete add-gallery
```

### Why one command

The user's intent is "I want to work on add-gallery". Whether it already exists is an implementation detail. One verb, one mental model.

### Switching safely

If you have unsaved work, Meow asks before switching. It can save it first or carry it along, explicitly. Either way, a snapshot exists.

### Combining honestly

`meow merge` always previews:

- what is coming in;
- what will change;
- whether it is a fast-forward;
- whether you will need to resolve anything.

It never surprises you with a merge commit you did not expect.

### Deleting a branch

Deleting an uncombined branch asks for confirmation and keeps a hidden copy for `meow recover`. Deleting a fully combined branch is routine.

---

## Secrets, large files, and ignoring

### The save guard

Before a save, Meow looks for:

- files that look like secrets (`.env`, private keys, known token formats);
- files above the large-file threshold (10 MB by default).

It stops, explains, and offers choices:

- ignore the file (`meow ignore`);
- save anyway (`--allow-secrets`, `--allow-large`);
- cancel.

This happens **before** anything enters history. That matters: removing a secret from history later is painful.

### If a secret is already saved

`meow ignore .env` stops future saves, but the old save still contains it. Meow says so and tells you to change the secret. It does not rewrite history automatically.

### Ignoring well

`meow ignore` with no path suggests:

- `node_modules/` for Node projects;
- `target/` for Rust;
- `.venv/`, `__pycache__/` for Python;
- `dist/` for built output;
- `.DS_Store`, `Thumbs.db` for operating-system clutter;
- `.env` for local secrets;
- editor folders such as `.vscode/` and `.idea/`.

---

## Plain mode and scripting

`--plain` is the stable API. It is also available as `MEOW_PLAIN=1`.

Properties:

- no prompts, ever;
- line-oriented, one fact per line;
- stable enough for scripts;
- exit code `2` when a decision is required.

Examples:

```sh
meow --plain save "Nightly checkpoint"
meow --plain changes --stat
meow --plain history
meow --plain doctor
meow --plain doctor --report
```

If a command cannot proceed without a person, it prints what it needs and exits `2`:

```text
  ❌ There are unsaved changes. (a decision is required)

  Save or discard them first, then run `meow merge`.
```

Other exit codes:

| Code | Meaning |
|---|---|
| `0` | Success, or nothing to do |
| `1` | The operation failed |
| `2` | A decision is required |

`--plain` also uses ASCII symbols, so output is safe in logs.

### Piping and colors

- Colors are disabled automatically when output is not a terminal.
- `NO_COLOR=1` disables color explicitly.
- `--ascii` replaces emoji even on a Unicode terminal.
- Long output is paged only in an interactive terminal; otherwise it prints directly.

---

## Configuration

Meow works with zero configuration. Optional settings live in `~/.config/meow/config.toml` (Windows: `%APPDATA%\meow\config.toml`).

```toml
editor = "code --wait"
large_file_mb = 10
default_branch = "main"
```

- `editor` — used by "Open in my editor" during a conflict. Falls back to `VISUAL`, then `EDITOR`, then `vi`/`notepad`.
- `large_file_mb` — the save guard's threshold. Default `10`.
- `default_branch` — used by `meow new` when Git has no `init.defaultBranch`.

Environment variables:

| Variable | Effect |
|---|---|
| `MEOW_PLAIN=1` | Same as `--plain` |
| `NO_COLOR=1` | Disable color |
| `VISUAL` / `EDITOR` | Editor fallback |
| `PAGER` | Pager for long diffs |

---

## Git interoperability

Meow is a front end, not a replacement.

- A Meow project is a normal Git repository.
- A teammate can `git clone` it, branch it, and push to it.
- CI can build it.
- Hooks run. Signing works. Credential helpers work. Attributes work.
- `meow --explain` prints the Git command it ran.
- `meow help --git` translates the whole vocabulary.

You can mix tools freely. Meow's undo journal only tracks Meow's own actions, but Git-level state (branches, commits, remotes) is shared.

### Translation table

| Meow | Git |
|---|---|
| `meow save` | `git add -A && git commit -m "…"` |
| `meow send` | `git push` (sets upstream on first send) |
| `meow get` | `git fetch && git merge`, or `git clone` |
| `meow changes` | `git diff HEAD` (plus a no-index diff for new files) |
| `meow history` | `git log`, `git show`, `git restore --source` |
| `meow undo` | `git reset --soft`, or `git revert` for a sent save |
| `meow branch` | `git branch`, `git switch -c`, `git switch` |
| `meow merge` | `git merge` (fast-forward when possible) |
| `meow resolve` | `git checkout --ours/--theirs`, `git add`, `git commit --no-edit` |
| `meow cancel` | `git merge --abort` |
| `meow discard` | `git restore` (untracked files move to Meow's trash) |
| `meow ignore` | edits `.gitignore`, then `git rm --cached -r` |
| `meow recover` | restores from private snapshots and trash |

---

## Troubleshooting

### "This folder isn't a project yet."

Run `meow new` in the folder, or `meow get <link>` to copy an existing project.

### "Meow needs a name and email to put on your saves."

```sh
meow doctor --fix
```

Or set them directly:

```sh
git config --global user.name "Your Name"
git config --global user.email "you@example.com"
```

### "Git isn't installed, or it isn't on your PATH."

- macOS: `brew install git`, or `xcode-select --install`.
- Windows: install from [git-scm.com/download/win](https://git-scm.com/download/win), then reopen the terminal.
- Linux: `sudo apt install git` (or your package manager).

### "A project check blocked your save."

A pre-commit hook failed. The hook's output is shown above the message. Fix what it reports, then save again. Meow does not bypass hooks.

### "Someone else sent work after your last get."

Run `meow get`, then `meow send` again. Meow never force-pushes.

### "A combine still needs your help."

Run `meow resolve`. To abandon the combine instead, run `meow cancel`.

### "Meow couldn't reach the online home."

Check your connection and credentials:

```sh
meow doctor
gh auth status
```

If the home moved, update the remote with Git or re-add it.

### "Your unsaved changes are still set aside safely."

This can happen if `meow get` cannot put your work back automatically. Nothing is lost:

```sh
meow resolve     # finish the combine, then Meow puts them back
meow doctor      # shows set-aside work
```

### A file I need is gone

```sh
meow recover
```

Look for a safety snapshot or a trash entry. `meow recover --purge` only removes copies; it never restores anything.

---

## Windows notes

Meow is tested with Windows Terminal, PowerShell, and `cmd`.

- Paths are displayed with `\` where Windows uses them.
- `notepad` is the editor fallback when neither `editor` nor `VISUAL`/`EDITOR` is set.
- `--ascii` and `--plain` are fully supported for terminals with limited Unicode.
- Long paths can be a problem in Git itself; enable `core.longpaths` if you hit it:

```sh
git config --global core.longpaths true
```

### CRLF line endings

Meow uses whatever Git is configured to use. If a team mixes Windows and Unix, set a `.gitattributes` or `core.autocrlf` as you normally would; Meow does not override it.

---

## When you outgrow meow

Meow is designed to make itself unnecessary.

- `meow --explain` shows every Git call.
- `meow help --git` is the translation table.
- `meow help <command>` names the Git concepts behind each action.
- The repository is normal, so you can open a terminal and use Git directly at any time.

If you find yourself reaching for a Git command Meow does not wrap, use Git. Meow will keep working on the same repository. There is no migration and no lock-in.

### Things Meow will not build

- A new version control system.
- A hosted service that owns your code.
- Force-push shortcuts.
- History rewriting behind your back.
- A self-updater.
- A walled garden that breaks `git clone`.

Those are deliberate choices, not missing features.
