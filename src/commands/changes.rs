use anyhow::Result;
use crate::ui::Ui;

pub fn run(ui: &Ui, path: Option<&str>, stat: bool, saved: Option<&str>) -> Result<i32> {
    let _ = (ui, path, stat, saved);
    Err(anyhow::anyhow!("That command is still being built."))
}
