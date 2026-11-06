use anyhow::Result;
use crate::ui::Ui;

pub fn run(ui: &Ui, command: Option<&str>, git: bool) -> Result<i32> {
    let _ = (ui, command, git);
    Err(anyhow::anyhow!("That command is still being built."))
}
