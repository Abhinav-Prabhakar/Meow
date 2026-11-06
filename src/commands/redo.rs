use anyhow::Result;
use crate::ui::Ui;

pub fn run(ui: &Ui) -> Result<i32> {
    let _ = ui;
    Err(anyhow::anyhow!("That command is still being built."))
}
