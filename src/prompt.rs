use std::io::{self, Write};

use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    queue,
    style::{Attribute, SetAttribute},
    terminal::{self, Clear, ClearType},
};
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;

const MAX_MATCHES: usize = 10;
const PROMPT_LABEL: &str = "Move to folder (empty=skip): ";

pub enum PromptResult {
    Input(String),
    Skip,
    Interrupted,
}

pub fn ask_destination(existing_dirs: &[String]) -> PromptResult {
    let mut stdout = io::stdout();
    let matcher = SkimMatcherV2::default();

    // Print prompt label BEFORE raw mode so it renders normally
    print!("{}", PROMPT_LABEL);
    let _ = stdout.flush();

    let _ = terminal::enable_raw_mode();

    let input_col = PROMPT_LABEL.len() as u16;
    let mut input = String::new();
    let mut selected: usize = 0;
    let mut prev_match_count: usize = 0;

    // Initial draw
    let matches = compute_matches(&matcher, &input, existing_dirs);
    prev_match_count = draw_relative(&mut stdout, input_col, &input, &matches, selected, prev_match_count);

    let result = loop {
        let evt = match event::read() {
            Ok(e) => e,
            Err(_) => break PromptResult::Interrupted,
        };

        match evt {
            Event::Key(KeyEvent {
                code: KeyCode::Char('c'),
                modifiers: KeyModifiers::CONTROL,
                ..
            })
            | Event::Key(KeyEvent {
                code: KeyCode::Esc, ..
            }) => {
                break PromptResult::Interrupted;
            }

            Event::Key(KeyEvent {
                code: KeyCode::Enter,
                ..
            }) => {
                let trimmed = input.trim().to_string();
                if trimmed.is_empty() {
                    break PromptResult::Skip;
                } else {
                    break PromptResult::Input(trimmed);
                }
            }

            Event::Key(KeyEvent {
                code: KeyCode::Tab, ..
            }) => {
                let matches = compute_matches(&matcher, &input, existing_dirs);
                if !matches.is_empty() && selected < matches.len() {
                    input = matches[selected].0.clone();
                }
                selected = 0;
                let matches = compute_matches(&matcher, &input, existing_dirs);
                prev_match_count = draw_relative(&mut stdout, input_col, &input, &matches, selected, prev_match_count);
            }

            Event::Key(KeyEvent {
                code: KeyCode::Backspace,
                ..
            }) => {
                input.pop();
                selected = 0;
                let matches = compute_matches(&matcher, &input, existing_dirs);
                prev_match_count = draw_relative(&mut stdout, input_col, &input, &matches, selected, prev_match_count);
            }

            Event::Key(KeyEvent {
                code: KeyCode::Up, ..
            }) => {
                if selected > 0 {
                    selected -= 1;
                }
                let matches = compute_matches(&matcher, &input, existing_dirs);
                prev_match_count = draw_relative(&mut stdout, input_col, &input, &matches, selected, prev_match_count);
            }

            Event::Key(KeyEvent {
                code: KeyCode::Down,
                ..
            }) => {
                let matches = compute_matches(&matcher, &input, existing_dirs);
                if !matches.is_empty() && selected < matches.len() - 1 {
                    selected += 1;
                }
                prev_match_count = draw_relative(&mut stdout, input_col, &input, &matches, selected, prev_match_count);
            }

            Event::Key(KeyEvent {
                code: KeyCode::Char(c),
                modifiers,
                ..
            }) if !modifiers.contains(KeyModifiers::CONTROL) => {
                input.push(c);
                selected = 0;
                let matches = compute_matches(&matcher, &input, existing_dirs);
                prev_match_count = draw_relative(&mut stdout, input_col, &input, &matches, selected, prev_match_count);
            }

            _ => {}
        }
    };

    // Clean up: move down past match lines, clear them, disable raw mode
    // We're on the prompt line; move down past all match lines and clear each
    for _ in 0..prev_match_count {
        let _ = write!(stdout, "\r\n");
        let _ = queue!(stdout, Clear(ClearType::CurrentLine));
    }
    // Move back up to prompt line
    if prev_match_count > 0 {
        let _ = queue!(stdout, cursor::MoveUp(prev_match_count as u16));
    }
    // Clear the prompt line and rewrite
    let _ = write!(stdout, "\r");
    let _ = queue!(stdout, Clear(ClearType::CurrentLine));
    let _ = stdout.flush();

    let _ = terminal::disable_raw_mode();

    // Reprint the prompt with final answer
    match &result {
        PromptResult::Input(s) => println!("{}{}", PROMPT_LABEL, s),
        PromptResult::Skip => println!("{}", PROMPT_LABEL),
        PromptResult::Interrupted => println!(),
    }

    result
}

fn compute_matches(
    matcher: &SkimMatcherV2,
    input: &str,
    dirs: &[String],
) -> Vec<(String, i64)> {
    if input.is_empty() {
        return dirs.iter().take(MAX_MATCHES).map(|d| (d.clone(), 0)).collect();
    }

    let mut scored: Vec<(String, i64)> = dirs
        .iter()
        .filter_map(|d| {
            matcher
                .fuzzy_match(d, input)
                .map(|score| (d.clone(), score))
        })
        .collect();

    scored.sort_by(|a, b| b.1.cmp(&a.1));
    scored.truncate(MAX_MATCHES);
    scored
}

/// Draws the input and match list using only relative cursor movements.
/// Returns the number of match lines drawn (for cleanup on next redraw).
fn draw_relative(
    stdout: &mut io::Stdout,
    input_col: u16,
    input: &str,
    matches: &[(String, i64)],
    selected: usize,
    prev_lines: usize,
) -> usize {
    // Go to start of input on prompt line (column after label)
    let _ = write!(stdout, "\r");
    let _ = queue!(stdout, cursor::MoveRight(input_col));
    let _ = queue!(stdout, Clear(ClearType::UntilNewLine));

    // Write the input text
    let _ = write!(stdout, "{}", input);

    // Draw match lines below
    let match_count = matches.len();
    let total_lines = match_count.max(prev_lines);

    for i in 0..total_lines {
        let _ = write!(stdout, "\r\n");
        let _ = queue!(stdout, Clear(ClearType::CurrentLine));
        if i < match_count {
            let (name, _score) = &matches[i];
            if i == selected {
                let _ = queue!(stdout, SetAttribute(Attribute::Reverse));
                let _ = write!(stdout, "  {} ", name);
                let _ = queue!(stdout, SetAttribute(Attribute::Reset));
            } else {
                let _ = write!(stdout, "  {} ", name);
            }
        }
    }

    // Move cursor back up to the prompt line
    if total_lines > 0 {
        let _ = queue!(stdout, cursor::MoveUp(total_lines as u16));
    }

    // Position cursor at end of input
    let _ = write!(stdout, "\r");
    let _ = queue!(stdout, cursor::MoveRight(input_col + input.len() as u16));

    let _ = stdout.flush();

    match_count
}
