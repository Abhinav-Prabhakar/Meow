use anyhow::Result;
use crate::ui::Ui;

pub fn run(ui: &Ui, purge: bool) -> Result<i32> {
    let _ = (ui, purge);
    Err(anyhow::anyhow!("That command is still being built."))
}
