# Start here

Meow saves your work, puts it online, and helps you get it back. You do not need to learn Git first.

## 1. Install

You need **Git 2.30+**. Rust is only needed if you build from source.

```sh
# macOS or Linux
brew install Abhinav-Prabhakar/tap/meow

# Windows (winget package in review)
winget install AbhinavPrabhakar.Meow
```

Or with Cargo, on any platform:

```sh
cargo install meow-git
```

Or build from source:

```sh
git clone https://github.com/Abhinav-Prabhakar/Meow.git
cd Meow
cargo install --path .
```

Check what you got:

```sh
meow version
```

Then check your setup:

```sh
meow doctor
```

## 2. Start a project

```sh
meow new portfolio
cd portfolio
```

Meow creates the folder, starts the project, and adds a sensible `.gitignore` so installed packages and secrets stay out.

## 3. Save

```sh
meow save "First draft"
```

A save is a checkpoint you can come back to. It stays on your machine until you send it.

See what changed since the last save:

```sh
meow changes
meow history
```

## 4. Send

```sh
meow send
```

If you have the GitHub CLI (`gh`) installed and signed in, Meow creates a repository and sends your work there. Otherwise it asks you to paste a repository link.

## 5. Get new work

```sh
meow get
```

That brings down new saves from your team. Unsaved work is set aside safely and put back afterwards.

On a new machine, copy a project with its link:

```sh
meow get github.com/ana/portfolio
```

## 6. Undo a mistake

```sh
meow undo
```

That is the everyday loop:

```text
  meow · ~/Projects/portfolio

  You're on main.

  ✅ No unsaved changes.

  Next
    meow send  put your project online
```

## If you get stuck

```sh
meow                  # where am I, what's next
meow help             # every command, in plain words
meow doctor           # check Git, your name, and your connection
meow recover          # find work you thought you lost
meow learn            # practice safely in a throwaway folder
```

Nothing in Meow is permanent until it is sent. Nearly every local mistake has a way back.

**Next:** the full guide is in [GUIDE.md](GUIDE.md).
