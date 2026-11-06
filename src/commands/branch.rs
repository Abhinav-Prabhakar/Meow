use anyhow::Result;
use crate::ui::Ui;

pub fn run(ui: &Ui, name: Option<&str>, delete: bool, force: bool) -> Result<i32> {
    let _ = (ui, name, delete, force);
    Err(anyhow::anyhow!("That command is still being built."))
}
