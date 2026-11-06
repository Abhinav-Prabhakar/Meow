use anyhow::Result;
use crate::ui::Ui;

pub fn run(ui: &Ui, verbose: bool) -> Result<i32> {
    let _ = (ui, verbose);
    Err(anyhow::anyhow!("That command is still being built."))
}
