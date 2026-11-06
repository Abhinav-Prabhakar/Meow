//! Command-line surface. Small vocabulary; Git words are hidden aliases.

use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(
    name = "meow",
    version,
    about = "Git, without the growl.",
    long_about = "Save your work, share it, and get it back — without learning Git first.",
    disable_help_subcommand = true,
    propagate_version = true
)]
pub struct Cli {
    /// Machine-readable output. Never prompts.
    #[arg(long, global = true)]
    pub plain: bool,

    /// Turn off color.
    #[arg(long, global = true)]
    pub no_color: bool,

    /// Use plain ASCII symbols instead of emoji.
    #[arg(long, global = true)]
    pub ascii: bool,

    /// Show the Git command behind each action.
    #[arg(long, global = true)]
    pub explain: bool,

    /// More detail when something goes wrong.
    #[arg(long, short = 'v', global = true)]
    pub verbose: bool,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Start a new project.
    #[command(alias = "init")]
    New {
        /// Folder to create, or the current folder if omitted.
        name: Option<String>,
    },

    /// Save a checkpoint of your work.
    #[command(alias = "commit")]
    Save {
        /// A short note describing the save.
        message: Option<String>,

        /// Save only these paths (advanced).
        #[arg(long, value_name = "PATH")]
        only: Vec<String>,

        /// Save even if something looks like a secret.
        #[arg(long)]
        allow_secrets: bool,

        /// Save even if a file is very large.
        #[arg(long)]
        allow_large: bool,
    },

    /// Put your saves online.
    #[command(alias = "push")]
    Send,

    /// Get the latest, or copy a project to this machine.
    #[command(alias = "pull", alias = "clone")]
    Get {
        /// A project link. Omit it to get the latest for this project.
        link: Option<String>,

        /// Folder to copy the project into.
        #[arg(long, value_name = "NAME")]
        folder: Option<String>,
    },

    /// See exactly what changed.
    #[command(alias = "diff")]
    Changes {
        /// Limit to one file or folder.
        path: Option<String>,

        /// Show only the summary.
        #[arg(long)]
        stat: bool,

        /// Show the changes in a past save instead.
        #[arg(long, value_name = "N")]
        saved: Option<String>,
    },

    /// See your past saves.
    #[command(alias = "log")]
    History {
        /// Show details for the Nth most recent save.
        n: Option<usize>,

        /// Include other branches.
        #[arg(long)]
        all: bool,

        /// Show exact times and short IDs.
        #[arg(long)]
        long: bool,

        /// Only saves that touched this file.
        #[arg(long, value_name = "PATH")]
        file: Option<String>,

        /// Bring the files from this save back as unsaved changes.
        #[arg(long, requires = "n")]
        restore: bool,
    },

    /// Take back your last action.
    Undo {
        /// How many actions to undo.
        n: Option<usize>,

        /// List recent actions.
        #[arg(long)]
        list: bool,
    },

    /// Put back what you just undid.
    Redo,

    /// List, start, or switch a branch.
    #[command(alias = "switch", alias = "checkout")]
    Branch {
        /// Branch name. Omit to list branches.
        name: Option<String>,

        /// Delete a branch instead of switching.
        #[arg(long)]
        delete: bool,

        /// Skip the safety confirmation when deleting uncombined work.
        #[arg(long)]
        force: bool,
    },

    /// Combine another branch into this one.
    #[command(alias = "combine")]
    Merge {
        /// Branch to combine into the current one.
        name: String,

        /// Combine without the preview.
        #[arg(long)]
        yes: bool,
    },

    /// Fix conflicts, one block at a time.
    Resolve {
        /// Keep your version everywhere.
        #[arg(long, conflicts_with_all = ["theirs", "both"])]
        ours: bool,

        /// Take their version everywhere.
        #[arg(long, conflicts_with_all = ["ours", "both"])]
        theirs: bool,

        /// Keep both versions everywhere.
        #[arg(long, conflicts_with_all = ["ours", "theirs"])]
        both: bool,

        /// Only list the files that need help.
        #[arg(long)]
        list: bool,
    },

    /// Stop a combine and go back.
    Cancel,

    /// Throw away unsaved changes.
    Discard {
        /// Files to throw away. Omit to choose interactively.
        paths: Vec<String>,

        /// Throw away every unsaved change.
        #[arg(long)]
        all: bool,
    },

    /// Stop tracking a file or folder.
    Ignore {
        /// The file, folder, or pattern to ignore. Omit to see suggestions.
        path: Option<String>,

        /// Show what is currently ignored.
        #[arg(long)]
        list: bool,
    },

    /// Find work you thought you lost.
    Recover {
        /// Permanently empty the trash and prune old snapshots.
        #[arg(long)]
        purge: bool,
    },

    /// Check that everything is set up.
    Doctor {
        /// Apply safe fixes with your consent.
        #[arg(long)]
        fix: bool,

        /// Write a redacted diagnostic report.
        #[arg(long)]
        report: bool,
    },

    /// Plain-language help and examples.
    Help {
        /// The command to explain.
        command: Option<String>,

        /// Show the Meow-to-Git translation table.
        #[arg(long)]
        git: bool,
    },

    /// Practice in a safe sandbox.
    Learn,

    /// Version information.
    Version {
        /// Include paths and configuration locations.
        #[arg(long)]
        verbose: bool,
    },

    /// Where am I, and what next? (same as running `meow`).
    #[command(hide = true)]
    Status,
}
