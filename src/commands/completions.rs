//! `meow completions` — tab-completion scripts for the daily commands.

use std::io::stdout;

use anyhow::Result;
use clap::CommandFactory;
use clap_complete::generate;

use crate::cli::Cli;
use crate::ui::Ui;

pub fn run(_ui: &Ui, shell: clap_complete::Shell) -> Result<i32> {
    let mut command = Cli::command();
    generate(shell, &mut command, "meow", &mut stdout());
    Ok(0)
}
