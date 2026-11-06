use anyhow::Result;
use crate::ui::Ui;

pub fn run(ui: &Ui, link: Option<&str>, folder: Option<&str>) -> Result<i32> {
    let _ = (ui, link, folder);
    Err(anyhow::anyhow!("That command is still being built."))
}
