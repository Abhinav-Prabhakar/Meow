//! Meow — Git, without the growl.

mod cli;
mod commands;
mod config;
mod git;
mod journal;
mod repo;
mod ui;
mod util;

use anyhow::Result;
use clap::{error::ErrorKind, Parser};

use crate::cli::{Cli, Command};
use crate::ui::{DecisionRequired, Ui};

fn main() {
    let args: Vec<String> = std::env::args().collect();

    // `meow --help` is the first screen for many people; use our own.
    if args.len() == 2 && (args[1] == "--help" || args[1] == "-h") {
        let ui = Ui::new(
            false,
            false,
            args.iter().any(|a| a == "--ascii"),
            false,
            false,
        );
        let _ = commands::help::run(&ui, None, false);
        return;
    }

    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(err) => {
            if matches!(
                err.kind(),
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion
            ) {
                print!("{err}");
                return;
            }
            let ui = Ui::new(false, false, false, false, false);
            let text = err.to_string();
            let first = text
                .lines()
                .next()
                .unwrap_or("That command doesn't look right.");
            let first = first.trim_start_matches("error: ");
            ui.failure(first);
            ui.next(&[("meow help", "see everything you can do")]);
            std::process::exit(2);
        }
    };

    let ui = Ui::new(cli.plain, cli.no_color, cli.ascii, cli.explain, cli.verbose);
    let code = match run(&cli, &ui) {
        Ok(code) => code,
        Err(err) => report(&ui, err),
    };
    std::process::exit(code);
}

fn run(cli: &Cli, ui: &Ui) -> Result<i32> {
    let Some(command) = &cli.command else {
        return commands::home::run(ui);
    };
    match command {
        Command::Status => commands::home::run(ui),
        Command::New { name } => commands::new::run(ui, name.as_deref()),
        Command::Save {
            message,
            only,
            allow_secrets,
            allow_large,
        } => commands::save::run(ui, message.as_deref(), only, *allow_secrets, *allow_large),
        Command::Send => commands::send::run(ui),
        Command::Get { link, folder } => commands::get::run(ui, link.as_deref(), folder.as_deref()),
        Command::Changes { path, stat, saved } => {
            commands::changes::run(ui, path.as_deref(), *stat, saved.as_deref())
        }
        Command::History {
            n,
            all,
            long,
            file,
            restore,
        } => commands::history::run(ui, *n, *all, *long, file.as_deref(), *restore),
        Command::Undo { n, list } => commands::undo::run(ui, *n, *list),
        Command::Redo => commands::redo::run(ui),
        Command::Branch {
            args,
            delete,
            force,
        } => commands::branch::run(ui, args, *delete, *force),
        Command::Merge { name, yes } => commands::merge::run(ui, name, *yes),
        Command::Resolve {
            ours,
            theirs,
            both,
            list,
        } => commands::resolve::run(ui, *ours, *theirs, *both, *list),
        Command::Cancel => commands::cancel::run(ui),
        Command::Discard { paths, all } => commands::discard::run(ui, paths, *all),
        Command::Ignore { path, list } => commands::ignore::run(ui, path.as_deref(), *list),
        Command::Recover { purge } => commands::recover::run(ui, *purge),
        Command::Doctor { fix, report } => commands::doctor::run(ui, *fix, *report),
        Command::Help { command, git } => commands::help::run(ui, command.as_deref(), *git),
        Command::Learn => commands::learn::run(ui),
        Command::Version { verbose } => commands::version::run(ui, *verbose),
    }
}

fn report(ui: &Ui, err: anyhow::Error) -> i32 {
    if let Some(d) = err.downcast_ref::<DecisionRequired>() {
        ui.failure(&d.what);
        if !d.hint.is_empty() {
            ui.blank();
            ui.hint(&d.hint);
        }
        return 2;
    }
    let text = format!("{err:#}");
    let mut lines = text.lines();
    if let Some(first) = lines.next() {
        ui.failure(first);
        for line in lines {
            ui.line(line);
        }
    }
    1
}
