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

const MAX_VISIBLE: usize = 10;
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
    let mut prev_drawn_lines: usize = 0;

    // Initial draw
    let matches = compute_matches(&matcher, &input, existing_dirs);
    prev_drawn_lines = draw_relative(&mut stdout, input_col, &input, &matches, selected, prev_drawn_lines);

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
                prev_drawn_lines = draw_relative(&mut stdout, input_col, &input, &matches, selected, prev_drawn_lines);
            }

            Event::Key(KeyEvent {
                code: KeyCode::Backspace,
                ..
            }) => {
                input.pop();
                selected = 0;
                let matches = compute_matches(&matcher, &input, existing_dirs);
                prev_drawn_lines = draw_relative(&mut stdout, input_col, &input, &matches, selected, prev_drawn_lines);
            }

            Event::Key(KeyEvent {
                code: KeyCode::Up, ..
            }) => {
                if selected > 0 {
                    selected -= 1;
                }
                let matches = compute_matches(&matcher, &input, existing_dirs);
                prev_drawn_lines = draw_relative(&mut stdout, input_col, &input, &matches, selected, prev_drawn_lines);
            }

            Event::Key(KeyEvent {
                code: KeyCode::Down,
                ..
            }) => {
                let matches = compute_matches(&matcher, &input, existing_dirs);
                if !matches.is_empty() && selected < matches.len() - 1 {
                    selected += 1;
                }
                prev_drawn_lines = draw_relative(&mut stdout, input_col, &input, &matches, selected, prev_drawn_lines);
            }

            Event::Key(KeyEvent {
                code: KeyCode::Char(c),
                modifiers,
                ..
            }) if !modifiers.contains(KeyModifiers::CONTROL) => {
                input.push(c);
                selected = 0;
                let matches = compute_matches(&matcher, &input, existing_dirs);
                prev_drawn_lines = draw_relative(&mut stdout, input_col, &input, &matches, selected, prev_drawn_lines);
            }

            _ => {}
        }
    };

    // Clean up: move down past drawn lines, clear them, disable raw mode
    for _ in 0..prev_drawn_lines {
        let _ = write!(stdout, "\r\n");
        let _ = queue!(stdout, Clear(ClearType::CurrentLine));
    }
    if prev_drawn_lines > 0 {
        let _ = queue!(stdout, cursor::MoveUp(prev_drawn_lines as u16));
    }
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
        return dirs.iter().map(|d| (d.clone(), 0)).collect();
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
    scored
}

/// Compute the visible window range around the selected index.
fn visible_window(matches_len: usize, selected: usize) -> (usize, usize) {
    if matches_len <= MAX_VISIBLE {
        return (0, matches_len);
    }
    let half = MAX_VISIBLE / 2;
    let start = if selected < half {
        0
    } else if selected + half >= matches_len {
        matches_len - MAX_VISIBLE
    } else {
        selected - half
    };
    (start, start + MAX_VISIBLE)
}

/// Draws the input and match list using only relative cursor movements.
/// Returns the number of lines drawn below the prompt (for cleanup on next redraw).
fn draw_relative(
    stdout: &mut io::Stdout,
    input_col: u16,
    input: &str,
    matches: &[(String, i64)],
    selected: usize,
    prev_lines: usize,
) -> usize {
    let (win_start, win_end) = visible_window(matches.len(), selected);
    let visible = &matches[win_start..win_end];

    let has_above = win_start > 0;
    let has_below = win_end < matches.len();

    // Count lines we'll draw: indicator lines + visible items
    let drawn_lines = (if has_above { 1 } else { 0 })
        + visible.len()
        + (if has_below { 1 } else { 0 });

    let total_lines = drawn_lines.max(prev_lines);

    // Go to start of input on prompt line
    let _ = write!(stdout, "\r");
    let _ = queue!(stdout, cursor::MoveRight(input_col));
    let _ = queue!(stdout, Clear(ClearType::UntilNewLine));
    let _ = write!(stdout, "{}", input);

    // Draw lines below
    let mut lines_written = 0;

    for i in 0..total_lines {
        let _ = write!(stdout, "\r\n");
        let _ = queue!(stdout, Clear(ClearType::CurrentLine));

        if has_above && lines_written == 0 && i == 0 {
            let _ = write!(stdout, "  \u{2191} {} more", win_start);
            lines_written += 1;
        } else if lines_written < (if has_above { 1 } else { 0 }) + visible.len() {
            let vis_idx = lines_written - if has_above { 1 } else { 0 };
            if vis_idx < visible.len() {
                let abs_idx = win_start + vis_idx;
                let (name, _) = &visible[vis_idx];
                if abs_idx == selected {
                    let _ = queue!(stdout, SetAttribute(Attribute::Reverse));
                    let _ = write!(stdout, "  {} ", name);
                    let _ = queue!(stdout, SetAttribute(Attribute::Reset));
                } else {
                    let _ = write!(stdout, "  {} ", name);
                }
            }
            lines_written += 1;
        } else if has_below && lines_written == (if has_above { 1 } else { 0 }) + visible.len() {
            let remaining = matches.len() - win_end;
            let _ = write!(stdout, "  \u{2193} {} more", remaining);
            lines_written += 1;
        }
        // else: clearing leftover lines from prev_lines
    }

    // Move cursor back up to the prompt line
    if total_lines > 0 {
        let _ = queue!(stdout, cursor::MoveUp(total_lines as u16));
    }

    // Position cursor at end of input
    let _ = write!(stdout, "\r");
    let _ = queue!(stdout, cursor::MoveRight(input_col + input.len() as u16));

    let _ = stdout.flush();

    drawn_lines
}
