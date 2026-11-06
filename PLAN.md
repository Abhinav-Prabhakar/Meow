# Meow — Product Plan

**Git, without the growl.**

A beginner-first command line tool that saves, shares, and protects your work — using real Git underneath.

| | |
|---|---|
| **Status** | Draft 1 — for review. No architecture or code decisions yet. |
| **Date** | 6 November 2026 |
| **Platforms** | macOS, Linux, Windows (first-class) |
| **Language** | Rust (single static binary, no runtime) |
| **Compatibility** | Standard `.git` repositories. Any Git host. Interoperable in both directions. |
| **Audience** | People who have never used Git, or tried and quietly gave up |

> This document specifies **behavior and product decisions**, not architecture. References to Git commands describe the compatibility contract — what must be true in the repository — not how Meow will be implemented.

**How to read this:** §0–§3 make the case; §4–§5 define the model and vocabulary; §6–§11 specify every command; §12–§14 cover teaching, output, and failure; §15–§21 cover Git compatibility, non-goals, scope, metrics, risks, and open questions. The appendices hold the cheat sheet, a full transcript, a comparison, a feature checklist, and the questions to prototype before writing code.

---

## 0. One-page summary

Meow is a small, opinionated layer over Git for people who find Git too much. It keeps every Git guarantee — real commits, real branches, real remotes, real interoperability — but exposes only the handful of ideas a beginner actually needs:

> **Save your work. Share it. Get other people's work. Undo mistakes. That's it.**

There is no staging area to learn. There is no `--force`. There is no detached HEAD. There are no ten-line error messages copied from `git`. There is a home screen that tells you where you are and what to type next, and there is a safety net that makes almost every mistake recoverable.

Meow does not invent a new version control system. It uses real Git underneath, so a Meow project is a normal project: a teammate can clone it with `git clone`, a CI system can build it, and a user can graduate to plain Git whenever they want. `meow --explain` even shows the exact Git commands it ran, turning the tool into a gentle on-ramp rather than a walled garden.

**The whole product in one screen:**

```
  meow · ~/Projects/portfolio

  You're on main.

  ● 2 unsaved changes                    +48 -12
  ↑ 2 saves not sent yet
  ↓ 3 new saves from your team

  Next
    meow save "Add project photos"
    meow get

  meow help for all commands
```

**The bet:** beginners do not need fewer capabilities, they need fewer *concepts*, fewer *decisions*, better *defaults*, and a way *back*. Meow removes decisions, not power.

---

## 1. The problem

Git is a brilliant storage engine wrapped in a hostile first experience.

A beginner who wants to "put my project online" is asked to understand:

- the difference between the working tree, the staging area, and the repository
- what `add` does, and why `commit -a` is different, and when to use which
- what a branch is, what `HEAD` is, and why it is sometimes "detached"
- what `origin` is, what "upstream" is, why `push -u` is needed once
- why `pull` sometimes creates a merge commit and sometimes does not
- what a fast-forward is
- what to do when a pull is rejected, and why the suggested fix is not the fix
- how to escape a merge, rebase, or detached HEAD they did not mean to enter
- what `reset --hard` will destroy, and whether they just destroyed it

None of this is required to save and share files. It is required to *operate Git safely*. Meow's job is to carry that operational knowledge so the user does not have to.

### The three fears

Every beginner-facing decision in this document traces back to one of three fears:

1. **"I will lose my work."**
   → Nothing destructive happens without a recoverable snapshot, and there is a visible undo.
2. **"I will break something I don't understand."**
   → Dangerous states are impossible to enter accidentally, and every state has an exit.
3. **"I don't know what to type."**
   → The tool tells you the next command, in context, in plain words. Typing `meow` alone is always useful.

### Why existing tools do not solve it

- **Plain Git** is the thing being replaced.
- **GitHub Desktop** solves it with a GUI, but many beginners are learning in a terminal (courses, servers, remote machines, editors), and a GUI hides the model rather than teaching it.
- **lazygit / gitui** are excellent *for people who already know Git*. They make Git faster, not simpler; they still show the index, branches, stash, and reflog.
- **jj** is a genuinely different model, but it is a new VCS. It does not produce ordinary Git repositories by default in the way a beginner's team expects, and it asks the user to learn new concepts.
- **gitless** proved the "simplified Git" idea but is a research prototype, not a maintained, polished, cross-platform tool.

Meow's specific niche: **a terminal tool, Git-native, with a small opinionated vocabulary, designed around safety and teaching, with output good enough to be the reason you use it.**

---

## 2. Who it is for

### Primary: the first-year user

Has heard of Git. May have typed `git init` once. Uses a terminal because a tutorial told them to. Works on a laptop. Wants their project on GitHub so they can share it, submit it, or collaborate. Has never resolved a merge conflict and is not confident they could.

**What success feels like:** "I saved my work and put it online in under three minutes, and I understood every step."

### Secondary: the returning user

Used Git badly for a year. Knows `add`, `commit`, `push`, `pull` as a ritual, not a model. Has been burned by a merge conflict or a `reset --hard`. Wants guardrails, a clean history, and an undo button.

### Tertiary: the collaborator who does not want to be a Git expert

Designers, researchers, writers, and students working with one or two other people. They need to get changes, send changes, and occasionally work on a separate branch. They do not need rebase, submodules, or tags.

### Explicit non-audience

- Teams with complex branching policies, monorepos, or release engineering.
- People who need rebase, cherry-pick, submodules, LFS, or signed-release pipelines.
- Git power users looking for a faster TUI. (They have lazygit. Meow will feel restrictive, and that is fine.)

We are not trying to convert power users. We are trying to make the first two years not hurt.

---

## 3. Principles

These are ranked. When two conflict, the higher one wins.

1. **Never lose the user's work.**
   Every destructive operation takes a snapshot first. "Discard" goes to a trash that can be restored. Undo covers every reversible action. The safety net is not a setting; it is always on.

2. **One obvious way to do the common thing.**
   One command to save, one to send, one to get, one to undo. No flags required for the 90% case. No "you could also…" in the primary path.

3. **No concept the user does not need today.**
   The index, HEAD, refs, upstream tracking, fast-forward, and the object graph are implementation details. They may appear in `--explain` and `meow help --git` for the curious, never in the daily flow.

4. **The tool carries the knowledge, not the user.**
   Detect state and suggest the next action. If a command cannot proceed, say what happened, why, and the exact command that fixes it. Never print a raw Git error as the primary message.

5. **Safe by default, reversible by design.**
   The default option is always the non-destructive one. Confirmation dialogs are rare, but when they appear they explain consequences in plain language. There is no force flag.

6. **Git-compatible, not Git-adjacent.**
   A Meow repository is a Git repository. No proprietary format, no lock-in, no "export to Git". If Meow disappeared tomorrow, `git` would still work.

7. **Quietly beautiful.**
   Output is calm, aligned, and readable at 80 columns. Color carries meaning and is never the only signal. No emoji soup, no ASCII fireworks, no cuteness in error messages. One small cat, in one or two places, as a signature.

8. **Teach by showing.**
   Every action can be explained. `--explain` reveals the Git underneath. `meow help --git` maps every Meow word to its Git equivalent. The goal is for users to outgrow us, not depend on us.

9. **Fast enough to feel invisible.**
   `meow` (the home screen) should feel instant on a normal project. Network operations show honest progress. Nothing blocks the terminal without saying why.

10. **No Meow account, no telemetry, no Meow server.**
    Local work works offline, against any Git host, with no sign-up. Credentials stay in the user's existing credential store. No analytics are collected.

---

## 4. The mental model

Five ideas. That is the whole product.

### 1. Project

A folder Meow watches. Creating one is `meow new`; getting one is `meow get <link>`. A project has a history of saves and, usually, one online home.

### 2. Save

A checkpoint of every file in the project, with a short note. "Save" is the word. Never "commit". A save is something the user made, not something the tool did to them.

> A save is complete: it contains the state of all tracked files at that moment. There is no half-saved state.

### 3. Branch

A parallel version of the project where you can try something without affecting the main version. Meow shows the current branch as "you're on main" and only introduces the word *branch* when the user asks to work on one.

Branches exist because "try a risky idea without breaking what works" is a beginner need, not an advanced one.

### 4. Home

The online copy of the project. Not "remote", not "origin", not "upstream" — the place your project lives. `meow send` puts saves there; `meow get` brings saves back.

### 5. Undo

Nothing is final. Every local action can be undone, and work that was never saved can still be recovered. Undo is a first-class idea, not an advanced command.

### The daily loop

```
   edit files
       │
       ▼
   meow save  ──────►  meow send
       ▲                  │
       │                  ▼
   meow undo  ◄──────  meow get
```

### The three questions

The home screen (`meow`) exists to answer exactly three questions, in order:

1. **Where am I?** — project, branch, online home
2. **What is the state of my work?** — unsaved changes, unsent saves, new saves from others
3. **What should I do next?** — one or two suggested commands, never more

Everything else is one command away.

---

## 5. The words

Meow deliberately uses a small, plain vocabulary. These are the only terms the user must learn. Git equivalents are for the curious and for `meow help --git`.

| Meow word | Meaning | Git equivalent |
|---|---|---|
| **project** | The folder being tracked | repository / working tree |
| **save** | A checkpoint with a note | commit |
| **unsaved changes** | Edits since the last save | working tree changes / unstaged |
| **send** | Upload your saves to the home | push |
| **get** | Download saves from the home, or copy a project | pull / fetch + merge / clone |
| **home** | The online copy of the project | remote (typically `origin`) |
| **branch** | A parallel version of the project | branch |
| **merge** | Bring another branch's saves into yours; the screens often say "combine" | merge |
| **conflict** | The same lines changed in two places | merge conflict |
| **ignore** | Tell Meow to leave a file alone | `.gitignore` |
| **recover** | Get back work that was discarded or set aside | stash / reflog / trash |
| **undo** | Reverse your last action | reset / revert (chosen automatically) |

Words we never show in the daily flow: *index, stage, HEAD, detached, ref, object, SHA, origin, upstream, tracking, fast-forward, rebase, cherry-pick, stash, force, fetch, origin/main, non-fast-forward, fatal.*

A short ID (like `a1b2c3d`) appears only in `meow history`, `--plain`, and `--explain`, never as something the user is required to type. Commands that refer to a save accept a position instead: `meow history 3` means "the third most recent save." `meow undo 2` is different by design — it means "reverse my last two actions", because undo is about actions, not saves.

---

## 6. The command set

### 6.1 The shape of the surface

The command set is deliberately tiered so that the help screen never looks like a wall.

- **The everyday seven** — 95% of sessions use only these. They are the only commands shown on first run and in the default `meow help`.
- **The occasional nine** — surfaced when relevant, listed under "when you need them". A user can go months without typing most of these.
- **Help and learning** — `help`, `learn`, `version`, `redo`.

There are no single-letter aliases (too easy to collide and too easy to forget). The commands themselves are short: `save`, `send`, `get`, `undo`, `branch`, `merge`.

Commands are English verbs a beginner already knows. The only Git-flavored words we keep are *branch* and *merge*, because they are the words users will see on GitHub and in every tutorial, and both are ordinary English too.

### 6.2 The everyday seven

| Command | What it means | Git underneath | Typical use |
|---|---|---|---|
| `meow` | Where am I, and what next? | `status`, ahead/behind, log -1 | Open a session, orient yourself |
| `meow save [note]` | Save a checkpoint of my work | `add -A` + `commit` | After making changes |
| `meow send` | Put my saves online | `push` (+ set upstream) | End of a work session |
| `meow get` | Get the latest, or get a copy | `pull` / `clone` | Start of a session, new machine |
| `meow changes [path]` | Show exactly what changed | `diff` | Before saving, to review |
| `meow history [n]` | Show my past saves | `log` | "When did this change?" |
| `meow undo` | Take back my last action | `reset --soft`, `revert`, restore | "Oops" |

### 6.3 The occasional nine

| Command | What it means | Git underneath | When it appears |
|---|---|---|---|
| `meow new [name]` | Start a new project | `init` | Beginning |
| `meow branch [name]` | List branches, or start/switch to one | `branch`, `switch -c`, `switch` | Trying something risky |
| `meow merge <name>` | Combine another branch into mine | `merge` | Finishing a branch |
| `meow resolve` | Walk me through conflicts | merge resolution | A conflict happened |
| `meow cancel` | Abandon an in-progress combine | `merge --abort` | A merge got scary |
| `meow discard [path]` | Throw away unsaved changes | `restore`, `clean` | Abandoning an experiment |
| `meow ignore [path]` | Stop tracking a file or folder | `.gitignore`, `rm --cached` | `.env`, `node_modules`, build output |
| `meow recover` | Find work I thought I lost | stash / reflog / trash | Panic |
| `meow doctor` | Check that everything is set up | config/credential checks | First run, weird errors |

### 6.4 Help and learning

| Command | Purpose |
|---|---|
| `meow help [command]` | Plain-language help, examples, and the Git equivalent |
| `meow help --git` | Full Meow ↔ Git translation table |
| `meow learn` | A safe, interactive 10-minute tutorial in a sandbox folder |
| `meow redo` | Reverse an undo (kept quiet, surfaced only after an undo) |
| `meow version` | Version info, including the Git version in use |

### 6.5 Naming rules

A command name must be:

- a word a beginner already uses in everyday English
- 3–7 characters, typed without reaching
- unambiguous with every other command
- a verb where the command performs an action (`save`, `send`, `merge`), a noun where it lists a thing (`history`, `branch`)
- never an abbreviation the user has to decode (`st`, `co`, `rb`)

An unknown command gets a fuzzy match, never a bare "unrecognized command": `meow sav` → "Did you mean `meow save`?" followed by the one-line usage.

We accept a small set of Git words as **hidden aliases** so muscle memory does not punish migrants: `init` → `new`, `clone` → `get`, `commit` → `save`, `push` → `send`, `pull` → `get`, `status` → `meow`, `log` → `history`, `diff` → `changes`, `switch` and `checkout` → `branch`. They never appear in help. This is a bridge, not a second vocabulary. *(Decision pending — see §21.)*

### 6.6 The learning ladder

The command count looks larger than it feels because the user learns it in stages. At no point does anyone need to memorise the whole list.

| When | Commands | What they can do |
|---|---|---|
| **Day one** | `meow`, `save`, `send`, `get` | The entire core loop: orient, save, share, receive |
| **Week one** | `changes`, `history`, `undo` | Review, understand, and take back mistakes |
| **When needed** | `new`, `branch`, `merge`, `resolve`, `cancel` | Start a project, work in parallel, handle a conflict |
| **Rare rescue** | `discard`, `ignore`, `recover`, `doctor` | Clean up, exclude files, find lost work, fix setup |

A user who only ever learns the first row still has a working, safe, shareable project. Everything below it is there when life gets complicated.

---

## 7. Command reference — the everyday seven

Each spec lists what the user experiences, not how it is built. The Git underneath is shown for the compatibility contract.

---

### 7.1 `meow` — the home screen

**Usage:** `meow`

**Does:** Prints the home screen: project path, current branch, unsaved changes, unsent saves, incoming saves, and a short "Next" block. This is the default thing that happens when a user types the tool name, and it must always be useful — even outside a project, even offline, even mid-merge.

**What the user sees:** the dashboard in §13.2. It is the product's front door and its most important screen.

**Rules:**

- Never more than **two** suggested commands.
- Suggestions are state-driven, not random: no changes → suggest making one; changes → suggest `save`; saves not sent → suggest `send`; behind → suggest `get`; conflict → suggest `resolve`.
- Outside a project, the screen becomes a welcome: explain the two ways to start (`new`, `get`) and offer `learn`.
- Mid-merge, the banner changes to a clear state: "You're combining two branches. `meow resolve` to finish, `meow cancel` to stop." This takes priority over every other suggestion.
- In an existing Git repo that Meow has not seen before, it works immediately: "This project already uses Git. Meow can work with it — nothing to set up."

**Git:** `status --porcelain=v2 --branch`, `rev-list --count`, `log -1`. Read-only.

**Options:** `--plain` (see §13.5), `--explain` (print the Git commands used), `--no-color`.

---

### 7.2 `meow save [note]` — save a checkpoint

**Usage:**

```
meow save
meow save "Add project photos"
meow save --only src/ README.md -m "Fix the header"
```

**Does:** Saves every non-ignored change in the project — new, modified, deleted, renamed — as one checkpoint with a short note.

There is no staging area. The default is "save everything I can see", which is what a beginner means by "save". Partial saves exist as `--only` for the rare case, not as a concept to learn.

**If no note is given:**

- Interactive terminal: prompts with a suggested note derived from the changed files, pre-filled and editable. A one-line tip appears the first few times: *"A short phrase in the present tense works well — 'Add project photos'."*
- Non-interactive: uses a generated note (`Update 3 files`) rather than failing. CI and scripts must not hang.

**What the user sees:**

```
  Saving 3 changes…

  ✓ Saved "Add project photos"                  main · just now

  2 saves are ready to send.
  Next  meow send
```

If there is nothing to save, it is not an error:

```
  Nothing has changed since your last save.
  Last save: "Add project photos" · 4 minutes ago
```

**Safety checks (before the save counts):**

- **Likely secrets** — filenames (`.env`, `id_rsa`, `*.pem`, `credentials.json`) and content patterns (private key blocks, common API token shapes). Explains the consequence, offers ignore-and-save-the-rest as the default, and never silently includes them.
- **Large files** — warns above a threshold (default 10 MB, strong warning above 50 MB) because a 400 MB video makes every future `get` slow for everyone. Offers ignore / save anyway / cancel.
- **Files that look important but are ignored** — e.g. a `src/` folder matched by an over-broad pattern. Warns once.
- **Pre-commit hooks** — if they exist, they run. A failure is explained as "A project check blocked your save", with the hook's output shown in an indented block and a link to `meow doctor`. No `--no-verify` in help.

**Git:** `git add -A` (respecting `.gitignore`), `git commit -m`, hooks and signing respected as configured.

**Options:**

| Option | Meaning |
|---|---|
| `-m`, `--message <text>` | The note, without prompting |
| `--only <path>...` | Save only these paths (advanced, not in default help) |
| `--explain` | Show the Git command used |
| `--plain` | Script-friendly one-line output |

**Never:** splits a save across multiple commits, rewrites old saves, amends without asking, or commits ignored files.

---

### 7.3 `meow send` — put your saves online

**Usage:** `meow send`

**Does:** Uploads the current branch's new saves to the project's home. Sets up tracking automatically the first time. If the project has no home yet, `send` becomes the guided setup flow: it explains what an online copy is, offers to create one (GitHub first, other hosts later), and falls back to clear manual steps if the user prefers.

**What the user sees:**

```
  Sending 2 saves to github.com/ana/portfolio…

  ✓ Sent. Your team can see your work now.

  main is up to date.
```

**Failure states are the product.** Each one is a first-class screen, not a Git error:

| Situation | What Meow does |
|---|---|
| No remote configured | Offers to create the online copy, or link an existing URL. Never says "no upstream". |
| Not logged in / bad credentials | Explains in one sentence, offers the login flow, and falls back to `meow doctor`. Never dumps an HTTP 403. |
| Someone else sent first (rejected push) | "Someone shared new work after your last `get`. Your saves are safe." Offers `get` then `send`, and can do both in one guided action after confirmation. Never suggests force. |
| Protected branch | "`main` is protected on this host. Meow can put your saves on a new branch and send that instead." Explains the branch name it will use. |
| Nothing to send | Friendly, not an error: "Everything is already online." |
| No saves yet | "Save your work first — then I can put it online." Offers `meow save`. |
| Offline | "No connection. Your saves are safe here — try again when you're online." Exit code 1, no retry loop. |
| No permission (not a collaborator) | "You don't have permission to send here. Ask the owner to invite you, or make your own copy." Fork/PR flow is post-1.0. |

**Git:** `git push` (with `-u` on first send), `git remote add` during setup, credential helper reused from Git.

**Options:** `--plain`, `--explain`. No `--force`, ever.

---

### 7.4 `meow get` — get the latest, or get a copy

**Usage:**

```
meow get                  # in a project: bring in new saves from the home
meow get <link>           # outside a project: copy a project to this machine
```

One word, two obvious meanings, disambiguated by context. This is deliberately the same verb: from the user's point of view, both are "get the project's work onto my machine".

**In a project (`meow get`):**

1. Downloads new saves.
2. If nothing changed locally, applies them instantly and summarizes what arrived.
3. If there are unsaved local changes, **sets them aside automatically**, applies the incoming saves, then puts them back. The user is told this happened in one line, and the safety net keeps a copy.
4. If you have saves that haven't been sent yet, getting will combine the incoming work with yours. Meow says so before it happens, and anything that needs a decision routes to `meow resolve`.
5. If putting local changes back would conflict, it stops cleanly, keeps both sides, and routes to `meow resolve` with an explanation — never a half-applied working tree.

**What the user sees:**

```
  Getting the latest from github.com/ana/portfolio…

  ✓ Got 3 new saves.
    Last one: "Fix the mobile menu" — Sam · 2 hours ago
    2 files changed in src/

  You're up to date.
```

**Outside a project (`meow get <link>`):**

- Accepts HTTPS or SSH URLs, with or without `.git`, and a bare `owner/repo` shorthand when the host is known.
- Shows the destination folder before writing, and refuses to overwrite a non-empty folder.
- Runs the first-run setup (identity) only if needed.
- Ends with the same home screen, so the user immediately knows what they have and what to do next.

**Git:** `git fetch` / `git pull --no-rebase` (merge, never rebase), `git stash` for the automatic set-aside, `git clone`.

**Options:** `--plain`, `--explain`, `--folder <name>` (clone destination). No `--rebase`, no `--ff-only` exposed.

---

### 7.5 `meow changes [path]` — see exactly what changed

**Usage:**

```
meow changes
meow changes src/app.js
```

**Does:** Shows unsaved changes as a readable diff. This is the "review before you save" screen, and the answer to "what did I actually do?"

**Design requirements:**

- Unified diff by default; side-by-side when the terminal is at least 100 columns.
- Three lines of context, word-level highlighting within changed lines.
- Renames detected and shown as renames, not delete + add.
- New files are shown with their content (or a size summary for large or binary files), not omitted the way `git diff` omits untracked files.
- Binary files summarized in human terms: `images/hero.jpg · 340 KB → 1.2 MB` (and image dimensions when cheaply available).
- Long output pages through a pager, with a footer: `q quit · / search · meow save when you're happy`.
- Empty state is warm and useful, not a blank screen: "Nothing has changed since your last save. Last save: …"

**Summary header:**

```
  Changes · 3 files · +48 -12

  src/index.html
    @@ -12,7 +12,9 @@
    ...
```

**Git:** `git diff` (working tree vs last save), with rename detection. Read-only.

**Options:** `--stat` (summary only), `--saved <n>` (show a past save instead — advanced), `--plain`, `--no-color`.

---

### 7.6 `meow history [n]` — your past saves

**Usage:**

```
meow history          # the last 10 saves
meow history 3        # details of the third most recent save
```

**Does:** Shows a calm, reverse-chronological list of saves, grouped by day, with relative times and authors. `meow history <n>` opens one save: the note, who made it, when, the files it touched, and the option to see the diff.

**List design:**

```
  History · main

  Today
    ● Add project photos                    you · 2 minutes ago
    ○ Fix the mobile menu                   Sam · 2 hours ago

  Yesterday
    ○ Update README                         you · 4:12 pm
    ○ Start the gallery page                you · 11:03 am

  ↑ 2 saves not sent
```

- The filled dot marks the save the project is currently at.
- Times are relative by default; `--long` shows exact timestamps.
- Branch names appear only when they differ from the current one.
- Short IDs are hidden by default, shown with `--long` (and in `--plain`), for users who need them when talking to a Git-using teammate.
- Merge saves read as sentences: `Combined "add-gallery" into main`.
- `--all` includes other branches; the default stays focused.
- No ASCII graph spaghetti in v1. A simple, readable branch diagram is a post-1.0 enhancement.

**Detail view:**

```
  Save #3 · "Fix the mobile menu"
  Sam · 2 hours ago · main

  2 files changed · +14 -3
    src/nav.js        +12 -3
    src/styles.css     +2 -0

  [d] see the changes   [r] bring this version back   [b] back
```

**Bringing an old version back** restores those files as *unsaved changes* on the current branch. It does not rewrite history, does not move the branch, and does not create a detached state. The user reviews the result with `meow changes` and saves it as a new checkpoint if they want it. A snapshot is taken first, so the restore itself is undoable.

**Git:** `git log --date=relative`, `git show --stat`.

**Options:** `--all`, `--long`, `--file <path>`, `--plain`.

---

### 7.7 `meow undo [n]` — take it back

**Usage:**

```
meow undo        # undo the last action
meow undo 2      # undo the last two actions
```

**Does:** Reverses the last Meow action, using a journal of actions and their inverses. This is not "undo the last Git command" — it is "undo what I just did", which is what the user means.

`n` is a count of actions, not a position: `meow undo 2` reverses the last two actions. If the project was changed outside Meow since the action was recorded, undo refuses with a plain explanation rather than guessing — the journal and the repository must agree.

**Undoable actions:**

| Action | What undo does |
|---|---|
| `save` (not sent) | Removes the save, **keeps the changes** in the files. The default is the safe one. |
| `save` (already sent) | Never rewrites shared history. Offers to create a new save that reverses it, and says so explicitly. |
| `discard` | Restores from the snapshot, including files moved to trash. |
| `get` / merge | Returns to the exact pre-operation state, keeping a snapshot of what arrived. |
| `branch` create/switch | Returns to the previous branch; created branches are removed only if they hold no unique saves. |
| `ignore` | Removes the pattern and restores tracking, if that was the effect. |
| `send` | Can't be undone — the work is already online. Explains that nothing is lost locally, and suggests a new save if the content was wrong. |

Undoing a combine that has already been sent is not offered in v1: reversing a shared merge is a genuinely advanced operation. Meow says so plainly and points anyone working with a Git user to `git revert -m 1`, rather than risking a wrong result.

**Interactive flow for an unsent save:**

```
  Your last save was "Add project photos" — 2 minutes ago.
  It hasn't been sent yet, so it's safe to undo.

  What should happen to your changes?
    1  Keep them in my files          (recommended)
    2  Remove them too

  > 1

  ✓ Undone. Your work is still here — it's just not saved anymore.
  Changed your mind?  meow redo
  Next  meow save "A better note"
```

**For a sent save:**

```
  That save is already shared, so Meow won't rewrite history.
  Instead, it can make a new save that undoes it — safe for everyone.

  Create "Undo: Add project photos"?  [y/N]
```

**Multi-step undo** is allowed but capped by the journal (last 50 actions). Each step prints what it did. `meow redo` reverses it. When the journal is unavailable (e.g., the repo was used by plain Git, or `.git/meow` was removed), `undo` explains the limit honestly instead of pretending.

**Git:** `git reset --soft HEAD~1` (keep changes), `git revert --no-edit` (sent saves), `git restore` / snapshot restore for discards, `git switch -` for branches.

**Options:** `--list` (show recent actions and which are undoable), `--explain`.

---

### 7.8 `meow redo` — put it back

**Usage:** `meow redo`

**Does:** Re-applies the last undo, if nothing else has happened since. Surfaced in the "Next" line immediately after an undo ("Changed your mind? `meow redo`"). Not listed in default help; discoverable at the moment it is useful.

---

## 8. The safety net

This is the feature that makes the rest of the product safe to use. It is not a command; it is a property of every command.

### 8.1 What is protected

Before any operation that could change or remove local work — `get`, `merge`, `branch` (switch), `discard`, `undo`, `cancel` — Meow takes a **snapshot** of the working tree, including:

- modified tracked files
- deleted tracked files
- untracked files that are not ignored
- the current branch and position
- the state of any in-progress merge

Snapshots are stored in the repository's private local storage, deduplicated and compressed, so unchanged files cost almost nothing. They are local only and are never sent. The last 20 are kept, plus anything younger than 30 days; pruning is automatic and announced in `meow doctor`, not in the daily flow.

### 8.2 `meow recover`

**Usage:** `meow recover`

**Does:** Lists recoverable work in plain language and restores it.

```
  Recoverable work

  Before switching to add-gallery     12 minutes ago   4 files
  Discarded images/draft.png          1 hour ago       1 file
  Before getting from github.com/…     yesterday        2 files

  [1-3] restore   [v] view   [q] quit
```

Recovery is granular: restore everything, or just one file. It never overwrites current work silently — it shows what would change and asks. If a restore would conflict, the incoming version is saved as a copy (`.meow-recovered-…`) rather than choosing for the user.

Emptying the trash and purging old snapshots is explicit: `meow recover --purge`. It says exactly what will be removed and when it would have expired on its own.

### 8.3 The promises

- **Cancelling a command never leaves a half-state.** Ctrl-C during a network operation is safe; the next `meow` detects any interrupted operation and offers resume or cancel.
- **Discarded files go to a trash, not the void.** Untracked files are moved, not deleted. Emptying the trash is an explicit action in `meow recover`.
- **A conflict never strands the user.** The home screen always shows the exit. `meow cancel` always works.
- **No operation requires a force flag.** If something truly cannot be done safely, Meow explains why and stops.

### 8.4 Storage and privacy

Snapshots live in `.git/meow/` and `refs/meow/`, never in commits, never on the remote. They are not visible to teammates and are not part of the project's history. A user can delete `.git/meow` at any time; Meow keeps working, with undo history reset and a one-line notice.

---

## 9. Branches and combining

Branching is the one place where beginners most often get hurt: they switch with unsaved work, they lose track of where they are, or they merge and hit a conflict with no idea how to get out. Meow's branch UX is built around three rules:

1. **You always know which branch you're on.** The home screen and every save/send output name it.
2. **Switching never surprises you.** Unsaved work is either saved first or carried along — explicitly, with your consent.
3. **There is always an exit.** Cancel a combine at any point and return exactly to where you started.

### 9.1 `meow branch [name]` — list, start, or switch

**Usage:**

```
meow branch               # list branches
meow branch add-gallery   # start a new branch, or switch to it if it exists
meow branch delete add-gallery
```

**Why one command instead of `switch` + `create`:** the user's intent is "I want to work on add-gallery". Whether that branch already exists is an implementation detail. One command, one mental verb. If the name is close to an existing branch, Meow asks before creating a near-duplicate ("Did you mean `add-gallery`?"). Creating a branch is always announced, so a typo cannot silently fork the project.

Names are checked against Git's rules and normalized to a friendly slug: `Add Gallery` becomes `add-gallery`, and anything Git would reject is refused with a suggested alternative before anything is created.

**Listing:**

```
  Branches

  ● main               you're here · 12 saves
    add-gallery        3 saves · last used 2 days ago
    fix-menu           2 saves · fully combined into main

  meow branch <name> to start or switch
```

**Switching with unsaved changes:**

```
  You have unsaved changes.

  What should happen to them?
    1  Save them here first          (recommended)
    2  Bring them with me
    3  Cancel

  > 1
```

Saving first is the default because it is the only option that cannot lose or misplace work. "Bring them with me" is supported — it is the normal Git behavior — but it is a deliberate choice, not a surprise. If carrying them would conflict with the destination, Meow stops before changing anything.

**Deleting:** only fully combined branches can be deleted without extra ceremony. An uncombined branch requires an explicit confirmation that names the saves that would become hard to find, and the branch tip is snapshotted so `meow recover` can bring it back.

**Git:** `git branch`, `git switch -c`, `git switch`, `git branch -d` / `-D` (only after confirmation + snapshot).

---

### 9.2 `meow merge <name>` — combine another branch into mine

**Usage:** `meow merge add-gallery`

**Does:** Brings the saves from another branch into the current one, with a preview first. The command is `meow merge`; the screens say "combine" because it is plainer English.

**Preview:**

```
  Combine add-gallery into main?

  3 saves · 5 files changed · +212 -48
  Last save: "Finish the gallery layout" — you · 2 days ago

  [c] combine   [d] see the changes   [x] cancel
```

The preview is the teaching moment: it shows *what* will happen before it happens, including whether it will be a simple "fast-forward" (Meow says "main hasn't changed since you branched — this will simply move it forward") or a real combine. No jargon required. If the combine will need the user's help, the preview says so before anything starts: "This will need your help in 2 files."

**Flow:**

1. Require a clean working tree. If there are unsaved changes, offer to save them first (default) or cancel. This removes an entire class of confusing states.
2. Merge. If clean, summarize and offer to delete the now-combined branch.
3. If there are conflicts, switch to the conflict flow (§9.3) without losing anything.
4. After success: "Combined. `main` now has 3 new saves." with next steps (`meow send`, `meow branch delete add-gallery`).

**Git:** `git merge --no-ff` when a real combine is needed, fast-forward when possible (decided automatically and explained in words).

---

### 9.3 Conflicts: `meow resolve`, `meow cancel`

A conflict is the moment a beginner decides whether they can do this. Most tools show a file full of `<<<<<<< HEAD` and walk away. Meow treats conflict resolution as a guided conversation.

**On conflict:**

```
  ✗ 2 files need your help

  You and Sam changed the same parts of these files.
  Nothing is lost. Let's go through them one at a time.

  [s] start   [d] explain what happened   [x] cancel the combine
```

**Per-conflict screen:**

```
  File 1 of 2 · src/app.js

  Sam's version                    Your version
  ┌──────────────────────────────┬──────────────────────────────┐
  │ const theme = "light";       │ const theme = "dark";        │
  │ applyTheme(theme);           │ applyTheme(theme);           │
  └──────────────────────────────┴──────────────────────────────┘

  [k] keep mine   [t] take Sam's   [b] keep both
  [o] open in editor   [d] more context   [q] decide later
```

**Rules:**

- Resolution is **per conflict block**, not per file. Most conflicts are one or two blocks; deciding per file forces an all-or-nothing choice that is often wrong.
- Every choice is shown in plain words, with names ("Sam's version"), not `ours`/`theirs`, which are famously ambiguous after a rebase.
- "Keep both" leaves both versions in place, clearly marked with a comment or a visible separator appropriate to the file type. It never claims the result is valid code.
- "Open in editor" is always available for anything non-trivial, and the file is presented without raw markers where possible (the editor gets a clean three-way view if a merge tool is configured; otherwise markers are explained in a header comment).
- Binary files (images, PDFs, spreadsheets) are never shown as text: "This file can't be merged line by line. Keep yours or Sam's?" with sizes and dates.
- Progress is always visible: "File 1 of 2", never a bare list.
- `q` to decide later is allowed. The home screen keeps a persistent banner until the combine is finished or cancelled.
- When the last conflict is resolved, `meow resolve` shows a summary and asks to finish. It then offers to send.

**`meow cancel`** aborts the combine and restores the exact pre-combine state, including unsaved work that existed before it started (from the safety snapshot). It confirms once:

```
  Cancel this combine?

  Both versions are kept — nothing is deleted.
  You'll be back on main exactly as you were.

  [c] cancel the combine   [k] keep going
```

**Git:** `git merge` conflict state, `git checkout --ours/--theirs` per path (translated to named sides), `git merge --abort`, snapshot restore for pre-existing work.

---

## 10. Starting, ignoring, and discarding

### 10.1 `meow new [name]` — start a project

**Usage:**

```
meow new portfolio        # create ./portfolio and start tracking it
meow new                  # start tracking the folder I'm in
```

**Does:**

1. Creates the folder if needed.
2. Checks whether Git is installed and whether the user's name and email are configured; if not, runs the one-time setup with a one-sentence explanation of why a name is attached to saves.
3. Starts the project.
4. Detects the kind of project (Node, Rust, Python, etc.) and proposes a sensible `.gitignore` before the first save — the single highest-value moment to prevent `node_modules` from entering history forever.
5. If the folder is empty, writes a short `README.md` containing the commands a beginner needs. If the folder already has files, never overwrites anything.
6. Ends on the home screen with the next step.

**What the user sees:**

```
  Starting ~/Projects/portfolio

  ✓ Project started
  ✓ Added a .gitignore for a Node project (node_modules, .env, dist)
  ✓ Wrote a starter README.md

  Next
    meow save "First save"       save the starter files
    meow send                    put it online after the first save
```

`meow new` on a folder that is already a Git project is not an error: "This project is already set up. Here's where you are." — then the home screen.

**Git:** `git init`, optional `git config --global user.name/user.email` (with consent), `git add` + `git commit` only if the user asked for a first save. No hidden commits.

---

### 10.2 `meow ignore [path]` — leave this file alone

**Usage:**

```
meow ignore node_modules
meow ignore .env
meow ignore                    # show what's ignored, suggest common ones
meow ignore --list
```

**Does:** Adds the path to `.gitignore` so it is not saved, and if it is already tracked, stops tracking it while keeping the local file. That second part is the magic most beginners cannot do with Git: `git rm --cached` plus editing the right file plus not deleting the actual file.

**Suggestions:** with no arguments, detects the project type and common clutter (`node_modules`, `.venv`, `target/`, `dist/`, `.DS_Store`, `__pycache__`, editor folders) and offers a checklist. This is the difference between a clean repository and a 400 MB accident.

**Already-committed secret warning:**

```
  ! .env is already in your project's history

  Ignoring it now stops future saves, but older saves still contain it.
  If it holds a password or key, change that password now.
  Meow can't remove it from history safely on its own.
```

**Protections:** refuses to ignore `.git`, warns before a pattern that would match source files (`*.rs`, `src/`), and explains what a pattern like `*.log` actually matches.

**Git:** edits `.gitignore`; if tracked, `git rm --cached -r`, which stops tracking the file and leaves it on disk untouched.

---

### 10.3 `meow discard [path]` — throw away unsaved changes

**Usage:**

```
meow discard                 # choose what to throw away
meow discard src/experiment.js
```

**Does:** Reverts unsaved changes to the last save. Untracked files are moved to Meow's trash rather than deleted. Everything is recoverable for 30 days via `meow recover`, and the confirmation says so.

**Flow (no path):** shows the changed files with checkboxes, preselects nothing, requires at least one selection, and confirms with a summary. No typing the project name, no `--hard`, no ambiguity about what will survive.

**Wording:** "Throw away" and "discard" are used together; the screen always shows the resulting state ("These files will go back to how they were at your last save"). It never says "reset" or "clean".

**Git:** `git restore` for tracked files, `git clean -fd` only after moving to trash (implemented as a move, then an index update).

---

## 11. Sharing and hosting

### 11.1 The "home" concept

A project usually has exactly one home: the online copy. Meow finds it automatically from the Git remotes:

- one remote → that is the home
- a remote named `origin` → that is the home
- several → asks once, remembers the choice in `.git/meow/config`, and never touches the others
- none → `send` offers to create one

The UI always shows the home as a host and path (`github.com/ana/portfolio`), never as `origin`. The word "remote" does not appear in the daily flow.

Any Git host works for `get` and `send` — Meow only needs a URL, and supports HTTPS and SSH. Automatic repository creation is limited to hosts Meow has an integration with; everything else is linked manually.

### 11.2 Authentication

Authentication is the second-most-common beginner wall after conflicts. Meow's approach:

1. **Reuse Git's credential helper.** If `git push` already works on this machine, Meow works with no setup. If the GitHub CLI is installed and signed in, its token is reused before asking for anything.
2. **Prefer HTTPS.** SSH keys are a common failure point; HTTPS plus the OS credential store is the path of least resistance. SSH URLs still work when the user already has keys configured.
3. **One guided login** when needed, not a wall of options: a browser-based device flow for the host (GitHub first), with a clear fallback ("your organization may require a token").
4. **`meow doctor` can test and fix** the common cases: missing helper, expired token, SSH key permissions, wrong username, corporate proxy.
5. **Never store secrets in the project.** Tokens live wherever the OS/Git credential helper puts them, never in `.git/config` in plain text.

### 11.3 Creating the online copy (`meow send` with no home)

```
  This project isn't online yet.

  An online copy lets you back up your work and share it.
  Where should it live?

    1  GitHub          (recommended)
    2  I already made one — paste the link
    3  Not now

  > 1

  Name: ana/portfolio
  Private?  [y/N]

  ✓ Created github.com/ana/portfolio
  ✓ Sent 3 saves
```

Creating a repository requires a host API and a registered OAuth application. That is a product dependency, not just an implementation detail: it needs an app registration, a privacy policy, and a support story. **v1 ships with GitHub creation and manual linking; GitLab and Codeberg creation are post-1.0.** If creation is unavailable, the flow degrades to precise manual instructions with a pre-filled repository name — never a dead end.

### 11.4 What we do not do (yet)

- **No pull requests.** A beginner working alone does not need them; a beginner on a team will be taught by that team. `meow send` on a protected branch creates a branch and explains the next step, but does not open a PR in v1.
- **No forks.** If the user lacks permission, they are told to ask for access or make their own copy.
- **No multiple homes.** One home per project. Existing extra remotes are preserved but not shown.
- **No hosting accounts of our own.** Meow talks to the host the user already uses.

---

## 12. Learning and discovery

Meow should make itself unnecessary. Every learning feature exists to move the user toward understanding, not dependence.

### 12.1 `meow help [command]`

No arguments: the seven commands that matter, grouped, plus one line on how to go deeper. Not a 60-line man page.

```
  meow — save, share, and protect your work

  Everyday
    meow                  where am I, what's next
    meow save "note"      save a checkpoint
    meow send             put your saves online
    meow get              get the latest (or get a copy)
    meow changes          see what changed
    meow history          see past saves
    meow undo             take back your last action

  When you need them
    meow new              start a project
    meow branch           work on a separate version
    meow merge            combine two versions
    meow resolve          fix a conflict
    meow discard          throw away unsaved changes
    meow ignore           stop tracking a file
    meow recover          find work you thought you lost
    meow doctor           check your setup

  meow help <command>     details and examples
  meow help --git         what this is called in Git
```

Per command, the help page has four parts: one-sentence meaning, two or three examples, what happens under the hood (the Git command), and the two most common mistakes. `meow help save --git` prints just the mapping.

### 12.2 `meow help --git` and `--explain`

The compatibility promise is only credible if it is visible.

```
  Meow              Git
  meow save         git add -A && git commit -m "…"
  meow send         git push (sets upstream on first send)
  meow get          git fetch && git merge (or git clone)
  meow branch       git branch / git switch [-c]
  meow merge        git merge
  meow undo         git reset --soft HEAD~1, or git revert for sent saves
  meow discard      git restore / clean (with a trash copy)
```

`meow <any command> --explain` prints the exact Git command(s) it ran, with the real arguments, indented and dimmed after the result. This is both a trust feature and a teaching feature: a user who is curious can watch themselves learn Git without being forced to.

### 12.3 `meow learn` — a safe place to practice

A 10-minute interactive tutorial that runs in a throwaway sandbox (`~/meow-practice` or a temp folder), with a simulated home so `send`/`get` work without an account or network.

Chapters:

1. Save your work
2. See what changed
3. Undo a save
4. Try something on a branch
5. Combine it back
6. Fix a conflict (deliberately created)
7. Share and get (two simulated people)
8. Recover something you deleted

Rules: no real project is touched, no network is required, quitting is always allowed, and progress is not gamified with badges. At the end: "You know the whole tool. `meow help` if you forget."

### 12.4 `meow doctor`

A health check that sounds like a person, not a diagnostic dump.

```
  Checking your setup…

  ✓ Git 2.45 found
  ✓ Name and email set (Ana Lima)
  ✓ github.com is reachable
  ! Your login for github.com has expired

  Fix it:  meow doctor --fix
```

Checks: Git presence and a minimum version, identity, home reachability, credentials, credential helper, SSH key permissions when SSH is used, repository health (`fsck` on request), in-progress operations, shallow clone, submodules, LFS pointers, large objects already in history, case-insensitive filesystem collisions, long paths on Windows, hooks that are failing, clock skew, and disk space.

`--fix` applies safe fixes with consent. `--report` writes a redacted diagnostic file for support. `doctor` never uploads anything on its own.

### 12.5 `meow version`

Prints the Meow version, the Git version it is driving, and the platform. Useful in bug reports. `--verbose` adds paths and config locations.


## 13. Output design

Output is not decoration. For a beginner, the screen *is* the interface: it is where they learn what happened, whether it is safe, and what to do next. Meow's output has a small, strict design system so that every screen feels like the same product.

### 13.1 Visual language

**Palette.** Colors are semantic, adaptive to light and dark terminals, and never the only carrier of meaning.

| Role | Used for | Dark theme | Light theme |
|---|---|---|---|
| Accent (amber) | Wordmark, prompt caret, the "Next" arrow | `#F0A868` | `#B45309` |
| Success | Completed actions, additions | `#7FD18B` | `#2E7D32` |
| Attention | Warnings, things needing a decision | `#F2CC60` | `#8A6100` |
| Danger | Failures, removals, destructive confirmations | `#F07C7C` | `#B3261E` |
| Info | Links, hosts, secondary facts | `#7CC7E8` | `#0B6285` |
| Muted | Labels, paths, timestamps, separators | dim (SGR 2) | dim |
| Base | Everything else | terminal foreground | terminal foreground |

Rules:

- Never hard-code black or white. Respect the terminal's own background.
- Never color a whole paragraph. Color marks a role, not emphasis.
- Diff additions and removals always carry a `+` or `-` prefix, so they read correctly without color.
- Maximum three colors on one line.

**Symbols.** One cell wide, consistent meaning, with an ASCII fallback.

| Unicode | Meaning | ASCII fallback |
|---|---|---|
| `✓` | done, safe, up to date | `[ok]` |
| `✗` | failed, cannot proceed | `[x]` |
| `!` | needs attention | `[!]` |
| `●` | current position | `*` |
| `○` | other position | `o` |
| `↑` | saves waiting to be sent | `^` |
| `↓` | new saves to get | `v` |
| `→` | next step | `->` |
| `·` | separator | `-` |
| `…` | working, truncated | `...` |

No emoji. They break alignment, render inconsistently across terminals, and read as unserious in a tool people trust with their work.

**Layout.**

- Two-space left gutter on every line.
- Content width capped at 76 columns; wraps rather than truncates anything important.
- One blank line between blocks, never two.
- A single thin rule (`─`, dim) under the header, and nowhere else.
- Labels are sentence case and muted; values are primary. `Saves not sent   2`, not `SAVES NOT SENT: 2`.
- Numbers are right-aligned in columns; paths truncate from the left, keeping the filename.
- The most important line is bold. Everything else is normal weight.

**Structure.** Every screen follows the same order:

1. **Header** — `meow · <context>` (project path, host, branch)
2. **State** — what is true right now, one line per fact
3. **Result** — what just happened, if this was an action
4. **Next** — one or two commands, never more
5. **Footer** — only when needed (`q quit`, `? help`)

### 13.2 The screens

**Home — a normal working state**

```
  meow · ~/Projects/portfolio

  You're on main.
  Home  github.com/ana/portfolio

  ● 2 unsaved changes                    +48 -12
  ↑ 2 saves not sent yet
  ↓ 3 new saves from your team

  Next
    meow save "Add project photos"
    meow get
```

**Home — mid-merge (the banner takes priority)**

```
  meow · ~/Projects/portfolio

  You're combining add-gallery into main.
  2 files still need your help.

  Next
    meow resolve      finish the combine
    meow cancel       stop and go back
```

**Home — outside a project**

```
  meow

  This folder isn't a project yet.

  Start something new        meow new
  Get a project from a link  meow get <link>
  Practice safely            meow learn
```

**Save**

```
  Saving 3 changes…

  ✓ Saved "Add project photos"                  main · just now

  2 saves are ready to send.
  Next  meow send
```

**Send**

```
  Sending 2 saves to github.com/ana/portfolio…

  ✓ Sent. Your team can see your work now.

  main is up to date.
```

**Get**

```
  Getting the latest from github.com/ana/portfolio…

  ✓ Got 3 new saves.
    Last one: "Fix the mobile menu" — Sam · 2 hours ago
    2 files changed in src/

  You're up to date.
```

**Changes**

```
  Changes · 3 files · +48 -12

  src/index.html
    @@ -12,7 +12,9 @@
    -  <h1>Hello</h1>
    +  <h1>Hello, Ana</h1>
    +  <p>Welcome to the portfolio.</p>

  Next  meow save "Update the homepage"
```

**History**

```
  History · main

  Today
    ● Add project photos                    you · 2 minutes ago
    ○ Fix the mobile menu                   Sam · 2 hours ago

  Yesterday
    ○ Update README                         you · 4:12 pm

  ↑ 2 saves not sent
```

**Branch list**

```
  Branches

  ● main               you're here · 12 saves
    add-gallery        3 saves · last used 2 days ago
    fix-menu           2 saves · fully combined into main
```

**Merge preview**

```
  Combine add-gallery into main?

  3 saves · 5 files changed · +212 -48
  Last save: "Finish the gallery layout" — you · 2 days ago

  [c] combine   [d] see the changes   [x] cancel
```

**Conflict**

```
  File 1 of 2 · src/app.js

  Sam's version                    Your version
  ┌──────────────────────────────┬──────────────────────────────┐
  │ const theme = "light";       │ const theme = "dark";        │
  │ applyTheme(theme);           │ applyTheme(theme);           │
  └──────────────────────────────┴──────────────────────────────┘

  [k] keep mine   [t] take Sam's   [b] keep both
  [o] open in editor   [d] more context   [q] decide later
```

**Doctor**

```
  Checking your setup…

  ✓ Git 2.45 found
  ✓ Name and email set (Ana Lima)
  ✓ github.com is reachable
  ! Your login for github.com has expired

  Fix it:  meow doctor --fix
```

**First run**

```
   /\_/\
  ( o.o )   meow
   > ^ <

  Save, share, and protect your work — without learning Git first.

  Let's check your setup.                    meow doctor
  Or practice in a safe sandbox.             meow learn

  Start a new project                        meow new
  Get a project from a link                  meow get <link>
```

One cat. Here, and optionally at the top of `meow help`. Nowhere else.

**A failed action**

```
  ✗ Can't send yet

  Someone shared new work after your last get.
  Your saves are safe — nothing was lost.

  Next
    meow get        get their work, then send yours
```

**Empty states** always keep the shape of a real screen and always offer one action:

```
  Changes · none

  Nothing has changed since your last save.
  Last save: "Add project photos" · 4 minutes ago
```

### 13.3 Progress and motion

- Network operations show a single-line spinner with a byte counter after 200 ms; before that, nothing (avoid flicker).
- Long operations print a plain log line every few seconds; no spinner in non-TTY output.
- Ctrl-C is always safe. The operation either did not start, completed, or is recorded as interrupted and offered for resume/cancel on the next run.
- No fake delays, no animated success banners, no clearing the screen except on the home screen. The user's scrollback is theirs.
- `--plain` disables all motion.

### 13.4 Tone

Calm, direct, second person, no blame. Meow is a careful colleague, not a mascot.

**Do**

- "Your work is safe."
- "Meow couldn't reach github.com. Check your connection, then try again."
- "That save is already shared, so Meow won't rewrite history."
- "Nothing was lost."

**Don't**

- "fatal:"
- "error: failed to push some refs"
- "You must…"
- "Invalid input."
- "Oops! Something went wrong!"

Rules:

- The first line of a failure says what happened in plain words, never a raw Git message.
- Never blame the user ("you forgot", "you broke"). Describe the state.
- No exclamation marks except a single, genuine success line.
- One cat reference maximum per screen, and never in an error or a conflict.
- Every error ends with an action, not an apology.

### 13.5 Plain mode and accessibility

`--plain` (and the `MEOW_PLAIN=1` environment variable) produces stable, scriptable output:

```
saved a1b2c3d main "Add project photos" 3 files +48 -12
sent main github.com/ana/portfolio 2 saves
```

No color, no Unicode symbols, no spinner, no prompts. If a decision is required in plain mode, Meow exits with code 2 and names the flag that supplies the answer, rather than hanging.

Other requirements:

- Honors `NO_COLOR`, `CLICOLOR=0`, `TERM=dumb`, and `--no-color`.
- `--ascii` swaps every symbol for its fallback, even when color is on.
- `MEOW_THEME=auto|dark|light` and a config setting; auto detects the terminal background where possible.
- Contrast targets: 4.5:1 for body text, 3:1 for large text and symbols, verified in both themes.
- Every meaning carried by color is also carried by a word or symbol.
- Reads correctly when piped to a screen reader; no cursor-addressed art in the primary flow.
- Widths computed from the actual terminal, with a sane 80-column fallback when unknown.
- Below 60 columns, the layout stacks vertically and keeps the "Next" block intact; nothing important is truncated.

---

## 14. Errors and empty states

An error is the product's most important screen, because it is the moment the user is most likely to quit. The anatomy is fixed:

1. **What happened** — one plain sentence, no jargon.
2. **Why** — one sentence, only if it helps.
3. **What to do** — one exact command, or a short menu.
4. **Reassurance** — one line, when work is at stake.

### 14.1 Error catalog

| Trigger | First line | Next |
|---|---|---|
| Not in a project | "This folder isn't a Meow project yet." | `meow new` / `meow get <link>` |
| No name/email configured | "Meow needs a name to put on your saves." | guided setup, once |
| Nothing to save | "Nothing has changed since your last save." | (not an error; info) |
| Nothing to send | "Everything is already online." | (not an error; info) |
| Send rejected (behind) | "Someone shared new work after your last get." | `meow get`, then `meow send` |
| Bad credentials | "Your login for github.com has expired." | `meow doctor --fix` |
| Offline | "Meow couldn't reach github.com." | retry when online |
| Conflict | "2 files need your help." | `meow resolve` / `meow cancel` |
| Protected branch | "main is protected on this host." | send on a new branch (guided) |
| No permission | "You don't have permission to send here." | ask owner / make a copy |
| Secret detected | "This looks like a private key." | ignore it / save anyway / cancel |
| Large file | "This file is 412 MB." | ignore it / save anyway / cancel |
| Detached HEAD (from Git) | "You're looking at an old save, not a branch." | `meow branch main` |
| Rebase in progress (from Git) | "This project is in the middle of a Git operation Meow doesn't handle." | finish with `git rebase`, or `meow doctor` |
| Repo locked | "Another Meow or Git is using this project." | wait, or `meow doctor` |
| Hook failed | "A project check blocked your save." | show hook output, `meow doctor` |
| Disk full | "There isn't enough space to save." | free space, then retry |
| Unsupported feature (submodule/LFS) | "This project uses submodules, which Meow doesn't manage yet." | explain, link to `meow help --git` |

Behind every screen, the full technical detail — including the raw Git output — is written to a local log (`~/.local/state/meow/log`). It is shown only with `--verbose` or attached to `meow doctor --report`. The primary message never leads with it.

### 14.2 The empty-state rule

Every list, diff, history, and recovery screen must be useful when empty. It says what the emptiness means, when the last relevant thing happened, and one action. A blank screen is never acceptable.

### 14.3 Exit codes

- `0` — success, including benign no-ops ("nothing to save").
- `1` — the user's request could not be completed, but the state is consistent and recoverable.
- `2` — usage error, or a decision was required in a non-interactive context.

Scripts depend on this; it is part of the contract.

---

## 15. Compatibility with Git

The compatibility promise is not "we can export to Git". It is: **a Meow project is a Git project at every moment.**

### 15.1 The contract

- **Standard repository.** `.git` directory, standard objects and refs, no custom format, no alternates, no replace refs.
- **Standard saves.** One `meow save` is exactly one commit, with the user as author and committer, the note as the message, no Meow trailer, no signature, no extra metadata. Merge commits have the normal two parents.
- **Standard branches.** Branches live in `refs/heads/*`, remote-tracking branches in `refs/remotes/*`. Meow never puts a user branch anywhere else.
- **Private state stays private.** The action journal and safety snapshots live under `.git/meow/` and `refs/meow/*`. They are never committed, never pushed, and never appear in normal branch listings. Deleting them loses undo history, not work.
- **Respect the user's Git.** `.gitignore`, `.gitattributes`, line-ending settings, hooks, commit signing, GPG/SSH configuration, and credential helpers all behave as they would with plain Git.
- **No config hijacking.** Meow changes global Git config only for the initial name/email setup, with consent. It never rewrites aliases, defaults, or hooks.
- **One home, others untouched.** Existing remotes are preserved. Meow reads them, never renames or deletes them.
- **Works from anywhere inside the project.** Running `meow` in a subfolder finds the project root automatically.
- **Graceful in foreign states.** Repositories created by Git, cloned by other tools, or left mid-rebase are detected and handled with an explanation, never corrupted.

### 15.2 Feature-by-feature behavior

| Git capability | Meow behavior |
|---|---|
| init, clone, commit, push, pull, fetch, branch, merge | First-class, simplified |
| Working tree + index | Working tree first-class; index hidden and always consistent |
| Stash | Automatic and invisible during `get`/switch; `recover` exposes the result |
| Rebase, cherry-pick | Not offered. If one is in progress from Git, Meow stops and explains |
| Detached HEAD | Never created. If found, shown as "looking at an old save" with a way back |
| Tags / releases | Not in v1; existing tags are preserved and not shown |
| Submodules | Detected; read-only warning; use Git for them |
| Git LFS | Detected; pointer files never silently committed; use Git for LFS |
| Multiple remotes | One home chosen; all others preserved |
| Worktrees | The main working tree is supported; linked worktrees are detected and explained, then managed with Git |
| Shallow clones | Supported; history-limited states explained |
| Hooks | Respected; failures translated with the hook output preserved |
| Commit signing | Respected; signing failures explained |
| Sparse checkout | Not supported; detected and explained |
| Unicode / spaces / symlinks / executable bits / case-insensitive FS | Supported and tested |
| Nested Git repositories | Detected and explained; never silently added as an embedded repository |
| Bare repositories | Usable as a home, never as a working project |

### 15.3 Interoperability test matrix

Every release must pass a two-way matrix, executed against a real Git binary:

| Flow | Meow → Git | Git → Meow |
|---|---|---|
| Create project | `git status`, `git log` clean | `git init` then `meow` detects it |
| Save | `git log` shows one normal commit | `git commit` then `meow history` shows it |
| Branch | `git branch` sees it, correct parent | `git switch -c` then `meow branch` sees it |
| Merge | `git log --graph` shows the expected shape | `git merge` then `meow history` reads it |
| Conflict | Git can resolve a Meow-started conflict | Meow resolves a Git-started conflict |
| Share | `git clone` of a Meow-pushed repo works | `git push` then `meow get` works |
| Undo | The resulting history is valid to Git | A Git reset is reflected honestly in Meow |

A release is blocked if any cell fails. This matrix is the single most important quality gate in the project.

### 15.4 Graduation

The user should be able to leave. `meow help --git` and `--explain` exist for that. Documentation will include a one-page "when you outgrow Meow" guide mapping each command to the Git workflow it hides. No lock-in, no proprietary concepts, no guilt.

---

## 16. What we will not build

Saying no is the product. These are explicit non-goals for v1, with the reason.

| Not building | Why |
|---|---|
| A staging area / partial staging as a concept | The number-one source of beginner confusion. `save --only` covers the rare need |
| Rebase, cherry-pick, interactive history editing | Powerful, dangerous, and unnecessary for the target user |
| Force push, in any form | It is how beginners lose other people's work. If the state cannot be fixed safely, Meow stops |
| Detached HEAD | An implementation detail that should never be a user-visible state |
| Stash as a user concept | Automatic set-aside during `get`/switch, exposed only as `recover` |
| Submodules, LFS, sparse checkout, worktrees as managed features | Detected and explained; use Git |
| Tags, releases, changelogs | Not needed to save and share; a team that needs them has Git |
| Pull requests, reviews, forks | Host-specific collaboration for teams, not solo beginners; post-1.0 |
| A GUI or TUI dashboard | The terminal output is the interface. A GUI is a different product |
| Accounts, cloud storage, telemetry | No server, no lock-in, no surveillance |
| Multiple remotes as a first-class idea | One home. Others are preserved, not managed |
| A new object format or VCS | Compatibility is the point |
| A plugin system | Surface area without beginner value |
| Themes marketplace, sound, ASCII art beyond one cat | Cuteness is a garnish, not a feature |
| Collaborator and repository settings management | Host-specific; Meow explains where to do it on the host's website |
| Internationalization in v1 | English first; strings centralized so translation is possible later |

---

## 17. Getting Meow

### 17.1 Distribution

- **One binary, no runtime.** Rust, statically linked where practical, no Node/Python/Ruby dependency. The only external requirement is Git itself — whether Meow requires a separate Git install or bundles one is an open question in §21. `meow doctor` checks it either way.
- **Install paths:** Homebrew (macOS/Linux), `winget`/Scoop (Windows), `cargo install meow`, a signed installer for macOS and Windows, and a `curl | sh` script for the brave. Every path pinned to a checksum.
- **Architectures:** macOS arm64/x86_64, Linux x86_64/arm64 (glibc and musl), Windows x86_64/arm64.
- **Shell completions** for bash, zsh, fish, and PowerShell, installed by default. Completions only offer the daily commands; the rest appear after typing `meow help`.
- **Man page** generated from the same help text, so it can never drift.
- **Settings are optional.** A single config file (`~/.config/meow/config.toml`) for theme, editor, large-file threshold, and default branch. No project-level config in v1; Meow works with zero configuration.
- **No self-update in v1.** `meow version` tells the user how they installed it and the upgrade command. Background updaters are a trust problem for a tool that touches source code.

### 17.2 First-run

No Meow account, no wizard longer than three questions. The first invocation:

1. Detects whether Git is installed. If not, gives one platform-specific install instruction and stops.
2. Checks name/email only when a save is first attempted, not at launch.
3. Offers `meow doctor` and, once it ships, `meow learn`, but does not force either.
4. Explains the three words: save, send, get. One screen, five lines, skippable forever (`MEOW_SEEN_WELCOME=1`).

### 17.3 Documentation

- A single-page quick start: install, `meow new`, `meow save`, `meow send`. Under 300 words.
- A "coming from Git" page: the translation table, plus what Meow deliberately hides.
- A "when you outgrow Meow" page.
- A troubleshooting page that mirrors the error catalog, written in the same voice.
- Every page is also available offline via `meow help`.

---

## 18. Scope and milestones

No dates yet; these are scope gates, each with an exit criterion. The ordering is chosen so the riskiest UX (conflicts, auth) is tested before polish.

### v0.1 — The core loop (internal alpha)

**In:** `meow`, `save`, `changes`, `history`, `undo` (unsent saves), `new`, the output design system, `--plain`, `--explain`, first-run setup, Git interop tests for create/save/branch.

**Exit:** a project created and saved entirely by Meow is indistinguishable from a normal Git repository in `git log`, `git status`, and `git fsck`; five internal users complete the loop without instructions.

### v0.2 — Sharing (private beta)

**In:** `get` (clone and pull), `send` (push), no-home publish flow with GitHub creation + manual fallback, credential reuse, `doctor` v1, rejected-push and auth error flows, automatic set-aside during `get`, basic conflict detection.

**Exit:** a first-time user can go from an empty folder to a shared repository and back on a second machine, unaided, in under five minutes.

### v0.3 — Branches and recovery

**In:** `branch`, `merge` with preview, `resolve` per conflict block, `cancel`, `discard` with trash, safety snapshots, `recover`, full `undo`/`redo`, mid-operation detection and resume.

**Exit:** five first-time users each resolve a deliberately created two-block conflict and cancel a merge, unaided. The destructive-path test suite (50 scenarios) passes with zero permanent data loss.

### v0.4 — Safety and polish

**In:** `ignore` with suggestions, secret and large-file guards, `doctor --fix`, themes/`--ascii`/`NO_COLOR`, plain-mode coverage of every command, completions, packaging for all platforms, crash-safe journal, snapshot pruning, accessibility audit, documentation.

**Exit:** install works on all target platforms from a clean machine; every screen passes the visual and tone checklist; p95 home-screen latency under 100 ms on a 10k-file project.

### v1.0 — Launch

**In:** `learn`, the documentation site, macOS/Windows signed installers, performance tuning on large repositories, the full two-way Git interop matrix, a security review of local snapshots, and a public bug tracker.

**Exit:** the success criteria in §19 are met in a supervised study of at least 12 target users.

### Post-1.0

- GitLab and Codeberg repository creation; PR proposal flow (`meow propose`).
- `meow who <file>` and history search ("when did this break?").
- A tasteful, optional branch diagram in `history --all`.
- Editor integrations (VS Code, JetBrains) built on the same vocabulary.
- i18n, starting with Spanish, Hindi, and Chinese.
- Team conventions file (`.meow.toml`) for protected paths and review hints.

---

## 19. Success criteria

These are measured in supervised usability sessions and the beta program (with consent), not telemetry.

### Comprehension

- After 10 minutes, a first-time user can explain **save**, **send**, **get**, and **undo** in their own words.
- They never need to learn the words *commit*, *push*, *pull*, *index*, or *HEAD* to complete the core loop.
- Given a screenshot of an error, they can name the next action.

### Task success

| Task | Target |
|---|---|
| Empty folder → saved project | 100% in under 60 seconds, no help |
| Saved project → shared online | 90% in under 3 minutes, no help |
| Second machine → working copy | 90% in under 2 minutes, no help |
| Resolve a two-block conflict | 80% in under 5 minutes, no help |
| Recover a discarded file | 100% in under 2 minutes, no help |
| Cancel a merge and return to the previous state | 100%, exact state |

### Safety

- Zero permanent data loss across a 50-scenario destructive test suite.
- Every destructive action is reversible for at least 30 days.
- No command can push a force update or rewrite shared history.
- A process kill at any point leaves the repository valid and the next run offers a clear recovery.

### Experience

- ≥90% of sessions use four or fewer distinct commands.
- Every error screen ends with an actionable command.
- No screen exceeds 80 columns or requires scrolling to find the next step.
- The home screen answers all three questions without another command.

### Performance

| Operation | Target |
|---|---|
| `meow` on 10k files | p95 < 100 ms |
| `meow` on 100k files | p95 < 300 ms |
| `meow save` (excluding hooks) | < 1 s on 10k files |
| First progress feedback on network ops | < 300 ms |
| Binary size | < 20 MB compressed |

### Compatibility

- 100% of the two-way interop matrix passes on macOS, Linux, and Windows.
- A repository can move between Meow and plain Git mid-task with no explanation needed.

---

## 20. Risks

| Risk | Impact | Mitigation |
|---|---|---|
| **Abstraction leaks** — conflicts and authentication are inherently complex | Users hit a wall exactly where the tool promised simplicity | Treat both as designed screens, not error paths; test with beginners before building further; keep `cancel` and `recover` always available |
| **Hidden behavior surprises** — automatic set-aside or auto-merge feels like magic | Loss of trust | Always announce what happened in one line; show it in `recover`; allow undo |
| **Snapshots contain secrets** the user chose not to commit | Local secret exposure | Snapshots live under `.git/meow` with owner-only permissions, are never pushed, are pruned, and can be purged with `meow recover --purge` |
| **Name collision or trademark** — `meow` is a common word and an existing npm package | Brand and distribution problems | Clear the name in package registries and trademark databases before launch; keep a fallback name shortlist |
| **OAuth app burden** — GitHub creation needs an app, privacy policy, and support | Blocks the "send with no setup" promise | Ship manual linking as a first-class fallback; isolate the API behind a host adapter |
| **Host API changes** | Publishing breaks | Adapter layer, versioned API calls, `doctor` detects and explains; never block local work |
| **Performance on large repositories** | The tool feels worse than Git | Stream status, avoid full-repo scans on every run, benchmark 100k-file repos from v0.1, cap expensive checks |
| **Windows terminal variance** | Broken output on a large beginner audience | Test Windows Terminal, PowerShell, cmd, and VS Code terminal; `--plain` and `--ascii` everywhere; no reliance on cursor addressing |
| **Git version drift** | Subtle incompatibility | Minimum Git version checked by `doctor`; interop matrix runs against oldest supported and latest |
| **Scope creep** | The simple tool stops being simple | The non-goals list is a contract; every new command must remove a beginner failure observed in testing |
| **Over-cuteness** | Undermines trust in a tool that touches work | One cat, two places; tone guide; no cuteness in errors or conflicts; test with professional users |
| **Users still need pull requests** | Teams outgrow the tool | Document the handoff; post-1.0 `meow propose`; never pretend to replace review workflows |
| **Journal corruption or manual edits** | Undo becomes wrong | Validate the journal, fall back to "undo unavailable" honestly, never guess |

---

## 21. Open questions

Decisions that need an answer before build starts. Each has a recommendation.

1. **Is the name clear?** `meow` is an existing npm package name and a common word. Check Homebrew, crates.io, winget, and trademark databases. *Recommendation: verify now; keep `meow` for the command if available, with a distinctive package name (`meow-cli`) and a wordmark.*
2. **How much cat?** *Recommendation: one ASCII cat on first run and at the top of `meow help`, nowhere else. No cat in errors, conflicts, or status.*
3. **GitHub repository creation in v1?** It is the single biggest "wow" moment, but it adds an OAuth app, a privacy policy, and a support surface. *Recommendation: yes for GitHub, with a fully first-class manual fallback; defer GitLab/Codeberg.*
4. **Hidden Git aliases?** Accepting `init`, `clone`, `commit`, `push`, `pull`, `status`, `log`, `diff`, `switch`, and `checkout` helps migrants and costs little. *Recommendation: yes, undocumented in help, listed in `meow help --git`.*
5. **`meow learn` in v1?** It is a differentiator and a support-cost reducer, but it is a second product to build. *Recommendation: v1 if the schedule allows, otherwise v1.1; do not ship a half-built tutorial.*
6. **Undo of sent saves?** *Recommendation: never rewrite shared history; offer a reversing save with explicit confirmation.*
7. **Automatic set-aside on `get`?** *Recommendation: yes, announced, with a safety snapshot and a guided restore.*
8. **Windows first-class at launch?** It is a large beginner audience with the most terminal variance. *Recommendation: yes, but treat its testing as a milestone gate, not an afterthought.*
9. **Default branch name?** *Recommendation: respect `init.defaultBranch` if set, otherwise `main`.*
10. **`save --only` in v1?** *Recommendation: ship it but keep it out of default help; it is the escape hatch for the no-staging rule.*
11. **License and openness?** *Recommendation: dual MIT/Apache-2.0, public repository from v0.1, contribution guide written for non-Git-experts (using Meow).*
12. **Self-update?** *Recommendation: none in v1; package managers only.*
13. **Does Meow require Git to be installed?** Requiring it guarantees exact behavior for hooks, signing, credential helpers, and configuration. But it puts an install step in front of the exact audience that may not have Git. Bundling or embedding Git raises size, licensing, and update questions. *Recommendation: require Git for v1, detect it on first run, and make installation a single guided step; revisit bundling for Windows after launch.*

---

## Appendix A — Cheat sheet

The one card we want people to stick to their monitor.

```
  THE EVERYDAY SEVEN

  meow                     where am I, what's next
  meow save "note"         save a checkpoint
  meow send                put your saves online
  meow get                 get the latest
  meow changes             see what changed
  meow history             see past saves
  meow undo                take back your last action

  WHEN YOU NEED THEM

  meow new                 start a project
  meow get <link>          copy a project to this machine
  meow branch <name>       start or switch a version
  meow merge <name>        combine it into yours
  meow resolve             fix a conflict, step by step
  meow cancel              stop a combine
  meow discard             throw away unsaved changes
  meow ignore <path>       stop tracking a file
  meow recover             find work you thought you lost
  meow doctor              check your setup

  HELP

  meow help                all commands
  meow help <command>      details
  meow help --git          the Git underneath
  meow --explain           show the Git command just run
```

---

## Appendix B — A day with Meow

A complete first project, including the two things that usually break people.

```
  ~ $ meow new portfolio
  Starting ~/Projects/portfolio
  ✓ Project started
  ✓ Added a .gitignore for a Node project
  ✓ Wrote a starter README.md

  Next  meow save "First save"   save the starter files
        meow send                put it online

  ~/Projects/portfolio $ # ...edits index.html and adds photos...
  ~/Projects/portfolio $ meow
  meow · ~/Projects/portfolio

  You're on main.
  ● 3 unsaved changes                    +48 -12

  Next  meow save "Add project photos"

  ~/Projects/portfolio $ meow save "Add project photos"
  Saving 3 changes…
  ✓ Saved "Add project photos"                  main · just now

  ~/Projects/portfolio $ meow send
  This project isn't online yet.
  Where should it live?
    1  GitHub          (recommended)
    2  I already made one — paste the link
    3  Not now
  > 1
  Name: ana/portfolio
  Private?  [y/N] n
  ✓ Created github.com/ana/portfolio
  ✓ Sent 3 saves
```

Later that week, a teammate sends a change:

```
  ~/Projects/portfolio $ meow get
  Getting the latest from github.com/ana/portfolio…
  ✓ Got 2 new saves.
    Last one: "Fix the mobile menu" — Sam · 2 hours ago
    2 files changed in src/
```

A conflict:

```
  ~/Projects/portfolio $ meow get
  Getting the latest from github.com/ana/portfolio…
  ! 2 files need your help
  You and Sam changed the same parts. Nothing is lost.
  [s] start   [x] cancel

  > s
  File 1 of 2 · src/app.js

  Sam's version                    Your version
  ┌──────────────────────────────┬──────────────────────────────┐
  │ const theme = "light";       │ const theme = "dark";        │
  └──────────────────────────────┴──────────────────────────────┘

  [k] keep mine   [t] take Sam's   [b] keep both   [o] open in editor
  > k

  File 2 of 2 · src/styles.css
  ...
  ✓ Conflict fixed. Everything is combined.
  Next  meow send
```

An "oops":

```
  ~/Projects/portfolio $ # ...edits the homepage...
  ~/Projects/portfolio $ meow save "Update the homepage"
  Saving 2 changes…
  ✓ Saved "Update the homepage"                 main · just now

  ~/Projects/portfolio $ meow undo
  Your last save was "Update the homepage" — 1 minute ago.
  It hasn't been sent yet, so it's safe to undo.

  What should happen to your changes?
    1  Keep them in my files          (recommended)
    2  Remove them too
  > 1

  ✓ Undone. Your work is still here — it's just not saved anymore.
  Changed your mind?  meow redo
  Next  meow save "A better note"
```

And the panic case:

```
  ~/Projects/portfolio $ # ...tries a draft image and decides against it...
  ~/Projects/portfolio $ meow discard images/draft.png
  ✓ Discarded. You can get it back for 30 days with `meow recover`.

  ~/Projects/portfolio $ meow recover
  Recoverable work
    Discarded images/draft.png          1 minute ago      1 file
  [1] restore  [q] quit
  > 1
  ✓ Restored images/draft.png
```

That is the whole product: four verbs, two escapes, and no fear.

---

## Appendix C — Comparison

| | Plain Git | GitHub Desktop | lazygit / gitui | jj | gitless | **Meow** |
|---|---|---|---|---|---|---|
| Interface | CLI | GUI | TUI | CLI | CLI | **CLI** |
| Backend | Git | Git | Git | own (Git export) | Git | **Git, natively** |
| Interoperable at all times | — | mostly | yes | partial | yes | **yes** |
| Beginner-first vocabulary | no | some | no | no | yes | **yes** |
| No staging area | no | no | no | yes | yes | **yes** |
| Safety net + undo | manual | partial | no | yes | no | **yes** |
| Guided conflict resolution | no | basic | no | partial | no | **yes, per block** |
| Beautiful terminal output | no | n/a | yes | yes | no | **yes** |
| Works offline, no account | yes | yes | yes | yes | yes | **yes** |
| Maintained / cross-platform | yes | yes | yes | yes | no | **yes** |
| Teaches the Git underneath | — | no | no | no | no | **yes (`--explain`)** |

Meow is not trying to beat Git at Git. It is trying to be the first tool that a beginner can use without being taught, while leaving a real Git repository behind.

---

## Appendix D — Feature inventory

Everything the document proposes, with its scope. This is the checklist for "did we cover all of it?"

### v0.1

- Home screen with three questions and two suggestions
- Save with note prompt and generated fallback
- Changes (unified diff, summary, empty state)
- History (grouped, relative time, detail view, bring an old version back as unsaved changes)
- Undo for unsaved saves
- New project, stack detection, ignore template, starter README
- First-run identity setup
- Output design system (color, symbols, layout, tone)
- `--plain`, `--no-color`, `--explain`
- Two-way Git interop for init/save/branch

### v0.2

- Get (pull) with automatic set-aside and restore
- Get (clone) with URL normalization and safe destination
- Send with upstream setup
- Publish flow: GitHub creation + manual link
- Credential reuse, login guidance, auth failure screens
- Rejected push, protected branch, offline, no-permission screens
- Doctor v1 (Git, identity, reachability, credentials)
- Conflict detection and the resolve entry point

### v0.3

- Branch list, create, switch with save-first/carry/cancel
- Branch delete with snapshot and confirmation
- Merge preview, fast-forward explanation, real combine
- Per-block conflict resolution, keep mine/theirs/both/open editor
- Binary conflict handling
- Cancel merge with exact restore
- Discard with selection, trash, and confirmation
- Safety snapshots before every risky operation
- Recover: list, view, restore all/one, purge
- Undo/redo across save, discard, get, merge, branch, ignore
- Mid-operation detection, resume, and cancel
- Interrupted-operation recovery

### v0.4

- Ignore with detection and suggestions
- Untrack an already-tracked file without deleting it
- Secret detection (names + content patterns)
- Large-file warning and thresholds
- Doctor fixes, report, and deeper checks (hooks, shallow, LFS, submodules, case collisions, clock skew)
- Themes, `--ascii`, `NO_COLOR`, `TERM=dumb`
- Plain mode for every command, exit-code contract
- Shell completions, man page
- Packaging: Homebrew, winget/Scoop, cargo, installers
- Crash-safe journal, snapshot pruning, permissions
- Accessibility and contrast audit

### v1.0

- `meow learn` interactive tutorial with simulated home
- Documentation site and offline help parity
- Windows/macOS signed installers
- Performance targets on 10k and 100k files
- Full two-way interop matrix
- Security review of local snapshots
- `meow help --git`, graduation guide

### Later

- `meow propose` (pull request flow)
- GitLab and Codeberg creation
- `meow who <file>` and search
- Optional branch diagram
- Editor integrations
- i18n
- Team conventions file
- `meow find` ("when did this break?")
- Notebook-aware diffs (ignore saved outputs, compare code only)

### Explicitly rejected

- Staging area, rebase, cherry-pick, force push, detached HEAD
- Stash as a concept, tags, releases, submodules, LFS, worktrees as managed features
- Accounts, cloud storage, telemetry, self-update
- GUI/TUI, plugins, themes marketplace
- Multi-remote management, fork/PR automation in v1
- A background autosave daemon — saves stay explicit; snapshots cover risky operations
- AI-written save notes — the note should be the user's own understanding; suggestions stay deterministic

---

## Appendix E — Questions to answer by prototyping

Some things can only be judged by building a small, fake screen and watching someone use it. Before v0.1, prototype and test:

1. **The home screen.** Does "Next" with two commands actually get people moving, or do they type `meow help` anyway?
2. **The save prompt.** Is a pre-filled generated note helpful or does it anchor people into bad messages?
3. **The conflict screen.** Does side-by-side at 80 columns work, or is per-block resolution only viable at 100+? Test on a laptop at default size.
4. **Undo wording.** "Keep them in my files" vs "Remove them too" — do users understand what is being removed?
5. **The cat.** Show two versions of the first-run screen to five people who are not developers and five who are; measure whether the tone changes trust.
6. **`--plain` in scripts.** Confirm the one-line format is stable enough to be worth documenting as an API.
7. **One home.** Does hiding multiple remotes ever block a real beginner workflow? Test with a fork.
8. **The GitHub publish flow.** Measure drop-off at every step with a real OAuth app, not a mock.

---

*End of draft. Next step: review the open questions in §21, settle the command names, and prototype the eight screens in Appendix E before any code is written.*
