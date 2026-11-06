use anyhow::Result;
use crate::ui::Ui;

pub fn run(ui: &Ui, paths: &[String], all: bool) -> Result<i32> {
    let _ = (ui, paths, all);
    Err(anyhow::anyhow!("That command is still being built."))
}
