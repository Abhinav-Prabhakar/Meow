# meow

**Git, without the growl.** A small, beginner-first Git CLI for people who find Git too much.

```text
  🐱  meow

  Save, share, and protect your work — without learning Git first.
```

[![Rust](https://img.shields.io/badge/rust-1.74%2B-orange.svg)](https://www.rust-lang.org)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Platforms](https://img.shields.io/badge/platforms-macOS%20%7C%20Linux%20%7C%20Windows-lightgrey.svg)](#install)
[![Git](https://img.shields.io/badge/needs%20git-2.30%2B-black.svg)](https://git-scm.com)
[![CI](https://github.com/Abhinav-Prabhakar/Meow/actions/workflows/ci.yml/badge.svg)](https://github.com/Abhinav-Prabhakar/Meow/actions/workflows/ci.yml)

Meow is a real Git client. It does not invent a new version control system, hide your history in a database, or lock you in. It drives the `git` already installed on your machine, so hooks, signing, credentials, and configuration all keep working — and a teammate can clone your project with plain `git` whenever they like.

What it removes is the *vocabulary*: the staging area, detached HEAD, `--force`, and ten-line error messages. What is left is a handful of verbs you can learn in ten minutes.

> **Save your work. Share it. Get other people's work. Undo mistakes. That's it.**

---

## Why meow exists

Git is excellent and almost universally required. It is also a wall of concepts for someone whose actual goal is "put my portfolio online".

Meow keeps the power and removes the decisions:

- **No staging area.** `meow save` saves your work. `meow changes` shows what is unsaved.
- **No scary words by default.** You get *save*, *send*, *get*, *branch*, *combine*. Git's names are one flag away (`meow help --git`).
- **A way back from almost everything.** Every risky action takes a safety snapshot first. Discarded files go to a trash, not the void.
- **One screen that tells you what to do next.** No 60-line man page on first run.
- **Scriptable.** `--plain` is a stable, prompt-free API. Exit code `2` means "a decision is required".
- **Accessible by default.** Semantic color only, `--ascii` fallbacks for every symbol, and no color-only meaning.

---

## Install

Meow needs **Git 2.30 or newer** on your `PATH`. Everything else is optional. `meow doctor` checks your setup and tells you exactly what is missing.

### Homebrew — macOS and Linux

```sh
brew install Abhinav-Prabhakar/tap/meow
```

### Windows

```powershell
# The winget package is in review; Scoop and the MSI below work today.
winget install AbhinavPrabhakar.Meow
```

Or with Scoop:

```powershell
scoop bucket add meow https://github.com/Abhinav-Prabhakar/scoop-meow
scoop install meow
```

Each release also includes an MSI, plus a portable `.zip` you can unpack anywhere.

### Cargo — any platform with Rust

```sh
cargo install meow-git
```

The package is named `meow-git`; the binary it installs is `meow`.

### Installer script

macOS and Linux:

```sh
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/Abhinav-Prabhakar/Meow/releases/latest/download/meow-git-installer.sh | sh
```

Windows PowerShell:

```powershell
powershell -ExecutionPolicy Bypass -c "irm https://github.com/Abhinav-Prabhakar/Meow/releases/latest/download/meow-git-installer.ps1 | iex"
```

### From source

```sh
git clone https://github.com/Abhinav-Prabhakar/Meow.git
cd Meow
cargo install --path .
```

### Direct downloads

Prebuilt archives for macOS (Intel and Apple silicon), Linux (x86_64 and arm64), and Windows are on the [Releases page](https://github.com/Abhinav-Prabhakar/Meow/releases). Every archive has a SHA-256 checksum.

Upgrade with the package manager you installed with. Meow has **no self-updater**.

### Shell completions

Tab completion covers the daily commands. Ask Meow to print the script for your shell and put it where the shell looks for completions.

```sh
# bash (Linux)
meow completions bash > ~/.local/share/bash-completion/completions/meow

# zsh
meow completions zsh > "${fpath[1]}/_meow"

# fish
meow completions fish > ~/.config/fish/completions/meow.fish

# PowerShell (add to your profile)
meow completions powershell | Out-String | Invoke-Expression
```

Restart the shell after installing the script.

### First check

```sh
meow doctor
```

```text
  Checking your setup
  ──────────────────────────────────────────────────────────

    ✅ Git 2.54.0 found
    ✅ Name and email set (Ana Lima)
    ✅ GitHub CLI is signed in
    ✅ Login helper configured (osxkeychain)
    ✅ No operation is half-finished
    ✅ 9.7 GB free on this disk

  Everything looks good.
```

---

## Quick start

Four commands are enough to do real work.

```sh
meow new portfolio        # start a project
cd portfolio
meow save "First draft"   # checkpoint your work
meow send                 # put it online
```

That is the whole beginner's guide. Everything else is here when you need it.

> Prefer a guided version? Run `meow learn` for a ten-minute practice session in a throwaway folder. Nothing real — no account, no network — is touched.

More detail: **[docs/START.md](docs/START.md)** · the full guide: **[docs/GUIDE.md](docs/GUIDE.md)**

---

## A tour

### Where am I?

Run `meow` with no arguments.

```text
  meow · ~/Projects/portfolio

  You're on main.

  ● 2 unsaved changes                    +48 -12
  ↑ 1 save not sent yet

  Next
    meow save "Add project photos"       save your changes
    meow send                            put them online

  meow help for all commands
```

### Save a checkpoint

```text
  Saving 2 changes…

  ✅ Saved "Add project photos"  main · just now

  Next
    meow send  put your project online
```

`meow changes` shows the exact lines that are unsaved:

```text
  Changes · 1 file · +1 -0
  ──────────────────────────────────────────────────────────

  diff --git a/src/app.js b/src/app.js
  index a106507..45c6e6b 100644
  --- a/src/app.js
  +++ b/src/app.js
  @@ -1,3 +1,4 @@
   export function greet(name) {
     return `Hello, ${name}!`;
   }
  +export const version = 2;
```

`meow history` lists your saves, grouped by day:

```text
  History · main
  ──────────────────────────────────────────────────────────

  Today
    ● Add greeting and notes                                 you · just now
```

### Send and get

`meow send` puts your saves online. It uses the GitHub CLI when it is available, and falls back to a pasted repository link.

```text
  Sending 1 save to github.com/ana/portfolio…

  ✅ Sent. Your team can see your work now.

  main is up to date.
```

`meow get` brings new saves down — or copies a project onto a new machine:

```sh
meow get                                  # latest for this project
meow get github.com/ana/portfolio         # copy it here
meow get github.com/ana/portfolio --folder site
```

Your unsaved work is **set aside**, never thrown away, and put back afterwards.

### Branches and combining

A branch is a parallel version of your project. `meow branch` lists them; `meow branch <name>` starts or switches to one.

```text
  Branches
  ──────────────────────────────────────────────────────────

    ● main                                             you're here · 1 save

  meow branch <name> to start or switch
```

`meow merge add-gallery` previews the combine before doing it:

```text
  Combine add-gallery into main
  ──────────────────────────────────────────────────────────

  1 save · 1 file changed · +1 -0
  Last save: "Add gallery" — you · just now

  main hasn't changed since you branched — this simply moves it forward.

  Combining 1 save…

  ✅ Combined. main now has 1 new save.

  add-gallery is kept until you delete it — nothing was lost.
```

If both sides changed the same lines, `meow resolve` walks through the conflict one block at a time and offers **keep mine**, **take theirs**, **keep both**, or **open in my editor**. `meow cancel` puts everything back exactly where it was.

### Undo, discard, recover

- `meow undo` takes back your last save or action — including a combine whose conflicts you just resolved. Sent saves are never rewritten; Meow offers a reversing save instead.
- `meow redo` puts back what you just undid.
- `meow discard` throws away unsaved changes. Tracked files are restored, untracked files move to Meow's trash.
- `meow recover` lists safety copies, discarded work, and unsaved changes Meow set aside while getting new saves, and brings them back. Run `meow recover 2` to bring back item 2 directly.

```text
  Recoverable work
  ──────────────────────────────────────────────────────────

  1  Before discarding unsaved changes      12 minutes ago · 4 files
  2  Discarded                              1 hour ago · 1 file
```

### When something looks wrong

```sh
meow doctor            # check Git, identity, GitHub, credentials, project health
meow doctor --fix      # set a missing name and email with consent
meow doctor --report   # write a redacted file you can share
meow help              # all commands, in plain words
meow help save         # examples, common mistakes, and the Git underneath
meow help --git        # the full translation table
```

`meow --explain` shows the real Git command behind each action, so the tool teaches you the thing it is hiding.

---

## Commands at a glance

| Everyday | What it does |
|---|---|
| `meow` | Where am I, what needs doing, what's next |
| `meow save "note"` | Save a checkpoint of everything that changed |
| `meow send` | Put your saves online |
| `meow get` | Get the latest, or copy a project here |
| `meow changes` | See exactly what changed |
| `meow history` | See past saves |
| `meow undo` | Take back your last action |

| When you need them | What it does |
|---|---|
| `meow new` | Start a project |
| `meow get <link>` | Copy a project to this machine |
| `meow branch <name>` | Start or switch a parallel version |
| `meow merge <name>` | Combine another branch into yours |
| `meow resolve` | Fix conflicts one at a time |
| `meow cancel` | Stop a combine and go back |
| `meow discard` | Throw away unsaved changes |
| `meow ignore <path>` | Leave a file or folder out of the project |
| `meow recover` | Find work you thought you lost |
| `meow doctor` | Check that everything is set up |
| `meow learn` | Practice in a safe sandbox |
| `meow help` | Plain-language help |
| `meow completions <shell>` | Print a tab-completion script |

Git users can use the familiar names too — `commit`, `push`, `pull`, `clone`, `diff`, `log`, `switch`, `checkout`, `combine`, and `init` are hidden aliases. Meow will not advertise them, but it will not fight you either.

---

## How it works

### Real Git underneath

Meow shells out to your installed `git`. It never reimplements object storage, merging, or authentication.

That means:

- your hooks run, including pre-commit checks;
- commit signing works exactly as configured;
- credential helpers and SSH keys are used normally;
- `git log`, `git diff`, and every other Git tool see a normal repository;
- graduating to plain Git is always possible, with no export step.

### The safety net

Before anything that could change or remove local work — `get`, `merge`, a branch switch, `discard`, `undo`, `cancel` — Meow takes a private snapshot of the working tree. Snapshots are local only and never uploaded.

- The last **20** snapshots are kept, plus anything younger than **30 days**.
- Discarded untracked files move to a private trash.
- `meow recover` restores either one; `meow recover <n>` picks it from the list without a prompt.
- `meow recover --purge` removes them permanently, after asking.

Sent history is never rewritten. If you undo a save that has already been sent, Meow offers a new save that reverses it instead.

### Plain mode is the script API

`--plain` (or `MEOW_PLAIN=1`) switches to stable, line-oriented output and **never prompts**.

```sh
meow --plain save "Nightly checkpoint"
meow --plain history
meow --plain changes --stat
```

If a command needs a decision it cannot make on its own, it exits with code **2** and prints the decision it needs. That makes Meow safe inside scripts, CI, and editor integrations.

### Accessibility

- `--ascii` replaces every emoji with a text fallback (`[ok]`, `[!]`, `[x]`, `=^..^=`).
- `NO_COLOR=1` or `--no-color` disables color.
- Color never carries meaning alone: additions and removals always have `+`/`-`, states always have a symbol and a word.
- Output is capped at a readable width and works in narrow terminals.

---

## Configuration (optional)

Meow works with zero configuration. When you want to change something, create `~/.config/meow/config.toml` (on Windows, `%APPDATA%\meow\config.toml`):

```toml
# Open this editor for "Open in my editor" during a conflict.
editor = "code --wait"

# Warn before saving a file larger than this.
large_file_mb = 10

# Used by `meow new` when your global init.defaultBranch is unset.
default_branch = "main"
```

`VISUAL` and `EDITOR` are respected when `editor` is unset.

---

## Development

```sh
cargo build
cargo test                 # end-to-end tests against real Git
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

The design and product contract live in [PLAN.md](PLAN.md). The end-to-end tests in [`tests/cli.rs`](tests/cli.rs) run the real binary against real repositories, including destructive paths.

Layout:

```text
src/
  cli.rs          command surface and hidden aliases
  main.rs         dispatch and error reporting
  git.rs          the Git runner and failure classification
  repo.rs         status, branches, history, remotes
  journal.rs      undo journal, safety snapshots, trash
  ui.rs           output system, symbols, prompts, pager
  commands/       one module per command
tests/cli.rs      end-to-end CLI tests
```

Meow is MIT licensed. See [LICENSE](LICENSE).
