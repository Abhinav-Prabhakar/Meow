use anyhow::Result;
use crate::ui::Ui;

pub fn run(ui: &Ui, n: Option<usize>, list: bool) -> Result<i32> {
    let _ = (ui, n, list);
    Err(anyhow::anyhow!("That command is still being built."))
}
