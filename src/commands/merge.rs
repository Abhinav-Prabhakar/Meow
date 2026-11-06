use anyhow::Result;
use crate::ui::Ui;

pub fn run(ui: &Ui, name: &str, yes: bool) -> Result<i32> {
    let _ = (ui, name, yes);
    Err(anyhow::anyhow!("That command is still being built."))
}
