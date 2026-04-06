use std::io::{self, Write};
use std::path::Path;

use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    queue,
    style::{Attribute, SetAttribute},
    terminal::{self, Clear, ClearType},
};
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;

const PROMPT_LABEL: &str = "Move to folder (empty=skip): ";
const RENAME_LABEL: &str = "Rename to: ";

pub enum PromptResult {
    Input(String),
    Skip,
    GoBack,
    Interrupted,
}

/// Compute max visible items based on terminal height.
/// Reserve rows for content above suggestions: header (1) + size (1) + blank (1) +
/// preview (~2) + blank (1) + hint (1) + prompt (1) + scroll indicators (2) = ~10.
fn max_visible() -> usize {
    let (_, term_h) = terminal::size().unwrap_or((80, 24));
    let available = (term_h as usize).saturating_sub(10);
    available.max(5) // at least 5 items
}

pub fn ask_destination(
    existing_dirs: &[String],
    destination: &Path,
) -> PromptResult {
    let mut stdout = io::stdout();
    let matcher = SkimMatcherV2::default();
    let max_vis = max_visible();

    // Mutable copy of dirs so renames take effect immediately
    let mut dirs: Vec<String> = existing_dirs.to_vec();

    print!("{}", PROMPT_LABEL);
    let _ = stdout.flush();

    let _ = terminal::enable_raw_mode();

    let input_col = PROMPT_LABEL.len() as u16;
    let mut input = String::new();
    let mut selected: usize = 0;
    let mut prev_drawn_lines: usize = 0;

    let matches = compute_matches(&matcher, &input, &dirs);
    prev_drawn_lines = draw_relative(&mut stdout, input_col, &input, &matches, selected, prev_drawn_lines, max_vis);

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
                let matches = compute_matches(&matcher, &input, &dirs);
                if !matches.is_empty() && selected < matches.len() {
                    input = matches[selected].0.clone();
                }
                selected = 0;
                let matches = compute_matches(&matcher, &input, &dirs);
                prev_drawn_lines = draw_relative(&mut stdout, input_col, &input, &matches, selected, prev_drawn_lines, max_vis);
            }

            Event::Key(KeyEvent {
                code: KeyCode::Backspace,
                ..
            }) => {
                input.pop();
                selected = 0;
                let matches = compute_matches(&matcher, &input, &dirs);
                prev_drawn_lines = draw_relative(&mut stdout, input_col, &input, &matches, selected, prev_drawn_lines, max_vis);
            }

            Event::Key(KeyEvent {
                code: KeyCode::Up, ..
            }) => {
                if selected > 0 {
                    selected -= 1;
                }
                let matches = compute_matches(&matcher, &input, &dirs);
                prev_drawn_lines = draw_relative(&mut stdout, input_col, &input, &matches, selected, prev_drawn_lines, max_vis);
            }

            Event::Key(KeyEvent {
                code: KeyCode::Down,
                ..
            }) => {
                let matches = compute_matches(&matcher, &input, &dirs);
                if !matches.is_empty() && selected < matches.len() - 1 {
                    selected += 1;
                }
                prev_drawn_lines = draw_relative(&mut stdout, input_col, &input, &matches, selected, prev_drawn_lines, max_vis);
            }

            // Left arrow: go back to previous file
            Event::Key(KeyEvent {
                code: KeyCode::Left,
                ..
            }) => {
                if input.is_empty() {
                    break PromptResult::GoBack;
                }
            }

            // Right arrow: skip (same as empty Enter)
            Event::Key(KeyEvent {
                code: KeyCode::Right,
                ..
            }) => {
                if input.is_empty() {
                    break PromptResult::Skip;
                }
            }

            // Ctrl+R: rename selected folder
            Event::Key(KeyEvent {
                code: KeyCode::Char('r'),
                modifiers: KeyModifiers::CONTROL,
                ..
            }) => {
                let matches = compute_matches(&matcher, &input, &dirs);
                if !matches.is_empty() && selected < matches.len() {
                    let old_name = matches[selected].0.clone();
                    if let Some(new_name) = rename_inline(&mut stdout, &old_name, prev_drawn_lines) {
                        if !new_name.is_empty() && new_name != old_name {
                            if let Ok(()) = crate::files::rename_subdir(destination, &old_name, &new_name) {
                                // Update local dirs list
                                if let Some(pos) = dirs.iter().position(|d| d == &old_name) {
                                    dirs[pos] = new_name;
                                }
                            }
                        }
                    }
                    // Redraw prompt and matches
                    let _ = write!(stdout, "\r");
                    let _ = queue!(stdout, Clear(ClearType::CurrentLine));
                    let _ = write!(stdout, "{}", PROMPT_LABEL);
                    selected = 0;
                    let matches = compute_matches(&matcher, &input, &dirs);
                    prev_drawn_lines = draw_relative(&mut stdout, input_col, &input, &matches, selected, prev_drawn_lines, max_vis);
                }
            }

            Event::Key(KeyEvent {
                code: KeyCode::Char(c),
                modifiers,
                ..
            }) if !modifiers.contains(KeyModifiers::CONTROL) => {
                input.push(c);
                selected = 0;
                let matches = compute_matches(&matcher, &input, &dirs);
                prev_drawn_lines = draw_relative(&mut stdout, input_col, &input, &matches, selected, prev_drawn_lines, max_vis);
            }

            _ => {}
        }
    };

    // Clean up
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

    match &result {
        PromptResult::Input(s) => println!("{}{}", PROMPT_LABEL, s),
        PromptResult::Skip => println!("{}", PROMPT_LABEL),
        PromptResult::GoBack => println!("{}← back", PROMPT_LABEL),
        PromptResult::Interrupted => println!(),
    }

    result
}

/// Inline rename: replaces prompt line with "Rename to: old_name", lets user edit, returns new name.
/// Returns None if cancelled (Esc/Ctrl+C).
fn rename_inline(
    stdout: &mut io::Stdout,
    old_name: &str,
    prev_drawn_lines: usize,
) -> Option<String> {
    // Clear match lines
    for _ in 0..prev_drawn_lines {
        let _ = write!(stdout, "\r\n");
        let _ = queue!(stdout, Clear(ClearType::CurrentLine));
    }
    if prev_drawn_lines > 0 {
        let _ = queue!(stdout, cursor::MoveUp(prev_drawn_lines as u16));
    }

    // Show rename prompt
    let _ = write!(stdout, "\r");
    let _ = queue!(stdout, Clear(ClearType::CurrentLine));
    let rename_col = RENAME_LABEL.len() as u16;
    let _ = write!(stdout, "{}", RENAME_LABEL);

    let mut name = old_name.to_string();
    let _ = write!(stdout, "{}", name);
    let _ = stdout.flush();

    loop {
        let evt = match event::read() {
            Ok(e) => e,
            Err(_) => return None,
        };

        match evt {
            Event::Key(KeyEvent {
                code: KeyCode::Esc, ..
            })
            | Event::Key(KeyEvent {
                code: KeyCode::Char('c'),
                modifiers: KeyModifiers::CONTROL,
                ..
            }) => {
                return None;
            }

            Event::Key(KeyEvent {
                code: KeyCode::Enter,
                ..
            }) => {
                let trimmed = name.trim().to_string();
                return Some(trimmed);
            }

            Event::Key(KeyEvent {
                code: KeyCode::Backspace,
                ..
            }) => {
                name.pop();
            }

            Event::Key(KeyEvent {
                code: KeyCode::Char(c),
                modifiers,
                ..
            }) if !modifiers.contains(KeyModifiers::CONTROL) => {
                name.push(c);
            }

            _ => continue,
        }

        // Redraw rename input
        let _ = write!(stdout, "\r");
        let _ = queue!(stdout, cursor::MoveRight(rename_col));
        let _ = queue!(stdout, Clear(ClearType::UntilNewLine));
        let _ = write!(stdout, "{}", name);
        let _ = stdout.flush();
    }
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

fn visible_window(matches_len: usize, selected: usize, max_vis: usize) -> (usize, usize) {
    if matches_len <= max_vis {
        return (0, matches_len);
    }
    let half = max_vis / 2;
    let start = if selected < half {
        0
    } else if selected + half >= matches_len {
        matches_len - max_vis
    } else {
        selected - half
    };
    (start, start + max_vis)
}

fn draw_relative(
    stdout: &mut io::Stdout,
    input_col: u16,
    input: &str,
    matches: &[(String, i64)],
    selected: usize,
    prev_lines: usize,
    max_vis: usize,
) -> usize {
    let (win_start, win_end) = visible_window(matches.len(), selected, max_vis);
    let visible = &matches[win_start..win_end];

    let has_above = win_start > 0;
    let has_below = win_end < matches.len();

    let drawn_lines = (if has_above { 1 } else { 0 })
        + visible.len()
        + (if has_below { 1 } else { 0 });

    let total_lines = drawn_lines.max(prev_lines);

    let _ = write!(stdout, "\r");
    let _ = queue!(stdout, cursor::MoveRight(input_col));
    let _ = queue!(stdout, Clear(ClearType::UntilNewLine));
    let _ = write!(stdout, "{}", input);

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
    }

    if total_lines > 0 {
        let _ = queue!(stdout, cursor::MoveUp(total_lines as u16));
    }

    let _ = write!(stdout, "\r");
    let _ = queue!(stdout, cursor::MoveRight(input_col + input.len() as u16));

    let _ = stdout.flush();

    drawn_lines
}
