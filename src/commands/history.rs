use anyhow::Result;
use crate::ui::Ui;

pub fn run(ui: &Ui, n: Option<usize>, all: bool, long: bool, file: Option<&str>, restore: bool) -> Result<i32> {
    let _ = (ui, n, all, long, file, restore);
    Err(anyhow::anyhow!("That command is still being built."))
}
