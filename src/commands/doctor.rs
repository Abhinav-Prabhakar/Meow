use anyhow::Result;
use crate::ui::Ui;

pub fn run(ui: &Ui, fix: bool, report: bool) -> Result<i32> {
    let _ = (ui, fix, report);
    Err(anyhow::anyhow!("That command is still being built."))
}
