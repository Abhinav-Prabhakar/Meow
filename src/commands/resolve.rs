use anyhow::Result;
use crate::ui::Ui;

pub fn run(ui: &Ui, ours: bool, theirs: bool, both: bool, list: bool) -> Result<i32> {
    let _ = (ui, ours, theirs, both, list);
    Err(anyhow::anyhow!("That command is still being built."))
}
