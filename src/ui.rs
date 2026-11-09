//! The look and feel of Meow: a small, calm, consistent output layer.
//!
//! Every screen follows the same shape: header, state, result, next step.
//! Emoji carry meaning, never decoration, and always have an ASCII fallback.

use std::env;
use std::fmt::Display;
use std::io::{self, IsTerminal, Write};
use std::process::{Command, Stdio};

use anyhow::Result;
use console::Style;
use dialoguer::{theme::ColorfulTheme, Confirm, Input, MultiSelect, Select};

/// Raised when a command needs a decision but cannot ask for one.
#[derive(Debug)]
pub struct DecisionRequired {
    pub what: String,
    pub hint: String,
}

impl Display for DecisionRequired {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.what)
    }
}

impl std::error::Error for DecisionRequired {}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Sym {
    Success,
    Warn,
    Error,
    Home,
    Cat,
    Dot,
    Circle,
    Up,
    Down,
}

#[derive(Clone, Copy)]
pub enum Paint {
    Green,
    Yellow,
    Red,
    Cyan,
    Dim,
    Bold,
}

pub struct Ui {
    pub plain: bool,
    pub ascii: bool,
    pub color: bool,
    pub explain: bool,
    pub verbose: bool,
    interactive: bool,
}

impl Ui {
    pub fn new(plain: bool, no_color: bool, ascii: bool, explain: bool, verbose: bool) -> Self {
        let env_plain = env::var("MEOW_PLAIN").map(|v| v == "1").unwrap_or(false);
        let plain = plain || env_plain;
        let term = env::var("TERM").unwrap_or_default();
        let dumb = term == "dumb";
        let stdout_tty = io::stdout().is_terminal();
        let stdin_tty = io::stdin().is_terminal();
        let no_color_env = env::var_os("NO_COLOR").is_some()
            || env::var("CLICOLOR").map(|v| v == "0").unwrap_or(false);
        let color = !no_color && !plain && !dumb && !no_color_env && stdout_tty;
        let interactive = !plain && !dumb && stdin_tty && stdout_tty;
        Ui {
            plain,
            ascii,
            color,
            explain,
            verbose,
            interactive,
        }
    }

    // ---------------------------------------------------------------- symbols

    pub fn sym(&self, s: Sym) -> &'static str {
        let ascii = self.ascii || self.plain;
        match (s, ascii) {
            (Sym::Success, false) => "✅",
            (Sym::Warn, false) => "⚠️",
            (Sym::Error, false) => "❌",
            (Sym::Home, false) => "☁️",
            (Sym::Cat, false) => "🐱",
            (Sym::Dot, false) => "●",
            (Sym::Circle, false) => "○",
            (Sym::Up, false) => "↑",
            (Sym::Down, false) => "↓",
            (Sym::Success, true) => "[ok]",
            (Sym::Warn, true) => "[!]",
            (Sym::Error, true) => "[x]",
            (Sym::Home, true) => "home",
            (Sym::Cat, true) => "=^..^=",
            (Sym::Dot, true) => "*",
            (Sym::Circle, true) => "o",
            (Sym::Up, true) => "^",
            (Sym::Down, true) => "v",
        }
    }

    // ----------------------------------------------------------------- styles

    pub fn paint(&self, text: impl Display, p: Paint) -> String {
        let text = text.to_string();
        if !self.color {
            return text;
        }
        let st = match p {
            Paint::Green => Style::new().green(),
            Paint::Yellow => Style::new().yellow(),
            Paint::Red => Style::new().red(),
            Paint::Cyan => Style::new().cyan(),
            Paint::Dim => Style::new().dim(),
            Paint::Bold => Style::new().bold(),
        };
        st.apply_to(text).to_string()
    }

    pub fn dim(&self, text: impl Display) -> String {
        self.paint(text, Paint::Dim)
    }

    pub fn bold(&self, text: impl Display) -> String {
        self.paint(text, Paint::Bold)
    }

    pub fn green(&self, text: impl Display) -> String {
        self.paint(text, Paint::Green)
    }

    pub fn yellow(&self, text: impl Display) -> String {
        self.paint(text, Paint::Yellow)
    }

    pub fn red(&self, text: impl Display) -> String {
        self.paint(text, Paint::Red)
    }

    pub fn cyan(&self, text: impl Display) -> String {
        self.paint(text, Paint::Cyan)
    }

    // ----------------------------------------------------------------- output

    pub fn line(&self, text: impl Display) {
        if self.plain {
            println!("{text}");
        } else {
            println!("  {text}");
        }
    }

    pub fn blank(&self) {
        println!();
    }

    pub fn header(&self, context: &str) {
        if self.plain {
            return;
        }
        println!(
            "  {} {}",
            self.paint("meow", Paint::Bold),
            self.dim(format!("· {context}"))
        );
        self.blank();
    }

    pub fn title(&self, text: &str) {
        if self.plain {
            println!("{text}");
        } else {
            println!("  {}", self.bold(text));
            println!(
                "  {}",
                self.paint(
                    "─".repeat(self.width().min(60).saturating_sub(2)),
                    Paint::Dim
                )
            );
        }
    }

    pub fn success(&self, text: impl Display) {
        if self.plain {
            println!("{text}");
        } else {
            println!("  {} {}", self.sym(Sym::Success), self.green(text));
        }
    }

    pub fn warning(&self, text: impl Display) {
        if self.plain {
            println!("warning: {text}");
        } else {
            println!("  {} {}", self.sym(Sym::Warn), self.yellow(text));
        }
    }

    pub fn failure(&self, text: impl Display) {
        if self.plain {
            eprintln!("error: {text}");
        } else {
            eprintln!("  {} {}", self.sym(Sym::Error), self.red(text));
        }
    }

    pub fn hint(&self, text: impl Display) {
        if self.plain {
            return;
        }
        self.line(self.dim(text));
    }

    /// A small label/value pair, aligned to the terminal.
    pub fn kv(&self, label: &str, value: impl Display) {
        let pad = 18usize.saturating_sub(display_width(label));
        self.line(format!(
            "{}{}{}",
            self.dim(label),
            " ".repeat(pad.max(1)),
            value
        ));
    }

    pub fn next(&self, items: &[(&str, &str)]) {
        if self.plain {
            for (cmd, _) in items {
                println!("next: {cmd}");
            }
            return;
        }
        self.blank();
        self.line(self.dim("Next"));
        let width = items
            .iter()
            .map(|(c, _)| display_width(c))
            .max()
            .unwrap_or(0)
            + 2;
        for (cmd, hint) in items {
            let pad = width.saturating_sub(display_width(cmd));
            if hint.is_empty() {
                self.line(format!("  {}{}", self.cyan(cmd), " ".repeat(pad)));
            } else {
                self.line(format!(
                    "  {}{}{}",
                    self.cyan(cmd),
                    " ".repeat(pad),
                    self.dim(hint)
                ));
            }
        }
    }

    pub fn explain(&self, command: impl Display) {
        if !self.explain {
            return;
        }
        if self.plain {
            println!("git: {command}");
        } else {
            println!("  {} {}", self.dim("→"), self.dim(command));
        }
    }

    // ------------------------------------------------------------------ input

    pub fn can_prompt(&self) -> bool {
        self.interactive
    }

    pub fn decision_required(&self, what: &str, hint: &str) -> anyhow::Error {
        anyhow::Error::new(DecisionRequired {
            what: what.to_string(),
            hint: hint.to_string(),
        })
    }

    pub fn select(&self, prompt: &str, items: &[String], default: usize) -> Result<usize> {
        if !self.can_prompt() {
            return Err(self.decision_required(
                &format!("{prompt} (a decision is required)"),
                "Re-run in a terminal, or pass the matching flag.",
            ));
        }
        let theme = meow_theme(self.color);
        Ok(Select::with_theme(&theme)
            .with_prompt(prompt)
            .items(items)
            .default(default)
            .interact()?)
    }

    pub fn confirm(&self, prompt: &str, default: bool) -> Result<bool> {
        if !self.can_prompt() {
            return Err(self.decision_required(
                &format!("{prompt} (a decision is required)"),
                "Re-run in a terminal, or pass the matching flag.",
            ));
        }
        let theme = meow_theme(self.color);
        Ok(Confirm::with_theme(&theme)
            .with_prompt(prompt)
            .default(default)
            .interact()?)
    }

    pub fn input(&self, prompt: &str, initial: &str) -> Result<String> {
        if !self.can_prompt() {
            return Err(self.decision_required(
                &format!("{prompt} (input is required)"),
                "Re-run in a terminal, or pass the value as a flag.",
            ));
        }
        let theme = meow_theme(self.color);
        let mut input = Input::<String>::with_theme(&theme).with_prompt(prompt);
        if !initial.is_empty() {
            input = input.with_initial_text(initial);
        }
        Ok(input.interact_text()?.trim().to_string())
    }

    pub fn multi_select(
        &self,
        prompt: &str,
        items: &[String],
        defaults: &[bool],
    ) -> Result<Vec<usize>> {
        if !self.can_prompt() {
            return Err(self.decision_required(
                &format!("{prompt} (a selection is required)"),
                "Re-run in a terminal, or pass the paths as arguments.",
            ));
        }
        let theme = meow_theme(self.color);
        Ok(MultiSelect::with_theme(&theme)
            .with_prompt(prompt)
            .items(items)
            .defaults(defaults)
            .interact()?)
    }

    pub fn pause(&self, message: &str) -> Result<()> {
        if !self.can_prompt() {
            return Ok(());
        }
        let theme = meow_theme(self.color);
        let _: String = Input::with_theme(&theme)
            .with_prompt(message)
            .allow_empty(true)
            .interact_text()?;
        Ok(())
    }

    // ------------------------------------------------------------------- misc

    pub fn width(&self) -> usize {
        if self.plain {
            return 80;
        }
        console::Term::stdout()
            .size_checked()
            .map(|(_, w)| w as usize)
            .filter(|w| *w >= 20)
            .unwrap_or(80)
    }

    /// True when results are being shown to a person at a terminal.
    pub fn is_terminal(&self) -> bool {
        self.interactive && io::stdout().is_terminal()
    }

    pub fn page(&self, content: &str) {
        if self.plain || !io::stdout().is_terminal() {
            print!("{content}");
            return;
        }
        let lines = content.lines().count();
        let height = console::Term::stdout()
            .size_checked()
            .map(|(h, _)| h as usize)
            .unwrap_or(24);
        if lines + 4 < height {
            print!("{content}");
            let _ = io::stdout().flush();
            return;
        }
        let pager = env::var("PAGER").unwrap_or_else(|_| "less -R".to_string());
        let mut parts = pager.split_whitespace();
        let Some(bin) = parts.next() else {
            print!("{content}");
            return;
        };
        let child = Command::new(bin)
            .args(parts)
            .env("LESS", "FRX")
            .stdin(Stdio::piped())
            .spawn();
        match child {
            Ok(mut child) => {
                if let Some(mut stdin) = child.stdin.take() {
                    let _ = stdin.write_all(content.as_bytes());
                }
                let _ = child.wait();
            }
            Err(_) => print!("{content}"),
        }
    }
}

fn meow_theme(_color: bool) -> ColorfulTheme {
    ColorfulTheme::default()
}

/// Display width that accounts for emoji and wide characters.
pub fn display_width(s: &str) -> usize {
    use unicode_width::UnicodeWidthStr;
    // Emoji presentation adds a width-2 cluster; unicode-width is good enough
    // for the small fixed vocabulary and falls back to char count otherwise.
    s.width()
}

/// Pad a string on the right to a visible width.
pub fn pad_right(s: &str, width: usize) -> String {
    let w = display_width(s);
    format!("{s}{}", " ".repeat(width.saturating_sub(w)))
}
