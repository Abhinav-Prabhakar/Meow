use anyhow::Result;
use crate::ui::Ui;

pub fn run(ui: &Ui, path: Option<&str>, list: bool) -> Result<i32> {
    let _ = (ui, path, list);
    Err(anyhow::anyhow!("That command is still being built."))
}
