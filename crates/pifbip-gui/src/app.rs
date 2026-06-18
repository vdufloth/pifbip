//! The iced application: setup screen, sorting screen, and preview pane.

use std::path::{Path, PathBuf};

use iced::widget::{button, column, container, image, row, scrollable, text, text_input, Space};
use iced::{ContentFit, Element, Font, Length, Subscription, Task};

use pifbip_core::{format_size, render_pdf_page, FileKind, Outcome, SortSession};

use crate::theme;
use crate::video::{VideoStream, VIDEO_H, VIDEO_W};

const TEXT_PREVIEW_BYTES: u64 = 64 * 1024;
const MAX_SUGGESTIONS: usize = 12;

#[derive(Clone)]
pub struct Flags {
    pub origin: Option<PathBuf>,
    pub destination: Option<PathBuf>,
    pub depth: u16,
}

#[derive(Debug, Clone)]
pub enum Message {
    // Setup screen
    OriginChanged(String),
    DestinationChanged(String),
    DepthChanged(String),
    BrowseOrigin,
    BrowseDestination,
    Start,
    // Sorting screen
    InputChanged(String),
    Confirm,
    Skip,
    GoBack,
    SuggestionUp,
    SuggestionDown,
    AcceptSuggestion,
    SelectSuggestion(usize),
    ArrowLeft,
    ArrowRight,
    Tick,
    Quit,
}

enum Screen {
    Setup,
    Sorting,
    Done,
}

enum PreviewState {
    Empty,
    Image(PathBuf),
    Pdf(PathBuf), // temp PNG to display and clean up
    Text(String),
    Video,
    Other(String),
}

pub struct App {
    screen: Screen,
    // setup fields
    origin_input: String,
    destination_input: String,
    depth_input: String,
    setup_error: Option<String>,
    // sorting state
    session: Option<SortSession>,
    input: String,
    suggestions: Vec<String>,
    selected: usize,
    status: String,
    preview: PreviewState,
    video: Option<VideoStream>,
    current_frame: Option<image::Handle>,
}

impl App {
    pub fn new(flags: Flags) -> (Self, Task<Message>) {
        let app = App {
            screen: Screen::Setup,
            origin_input: flags
                .origin
                .map(|p| p.display().to_string())
                .unwrap_or_default(),
            destination_input: flags
                .destination
                .map(|p| p.display().to_string())
                .unwrap_or_default(),
            depth_input: flags.depth.to_string(),
            setup_error: None,
            session: None,
            input: String::new(),
            suggestions: Vec::new(),
            selected: 0,
            status: String::new(),
            preview: PreviewState::Empty,
            video: None,
            current_frame: None,
        };
        (app, Task::none())
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::OriginChanged(s) => self.origin_input = s,
            Message::DestinationChanged(s) => self.destination_input = s,
            Message::DepthChanged(s) => self.depth_input = s,
            Message::BrowseOrigin => {
                if let Some(p) = rfd::FileDialog::new().pick_folder() {
                    self.origin_input = p.display().to_string();
                }
            }
            Message::BrowseDestination => {
                if let Some(p) = rfd::FileDialog::new().pick_folder() {
                    self.destination_input = p.display().to_string();
                }
            }
            Message::Start => match self.build_session() {
                Ok(session) => {
                    self.session = Some(session);
                    self.screen = Screen::Sorting;
                    self.setup_error = None;
                    self.refresh();
                }
                Err(e) => self.setup_error = Some(e),
            },
            Message::InputChanged(s) => {
                self.input = s;
                self.selected = 0;
                self.recompute_suggestions();
            }
            Message::Confirm => self.do_confirm(),
            Message::Skip => self.do_skip(),
            Message::GoBack => self.do_goback(),
            Message::ArrowLeft => {
                if self.input.is_empty() {
                    self.do_goback();
                }
            }
            Message::ArrowRight => {
                if self.input.is_empty() {
                    self.do_skip();
                }
            }
            Message::SuggestionUp => self.selected = self.selected.saturating_sub(1),
            Message::SuggestionDown => {
                if self.selected + 1 < self.suggestions.len() {
                    self.selected += 1;
                }
            }
            Message::AcceptSuggestion => {
                if let Some(sug) = self.suggestions.get(self.selected).cloned() {
                    self.input = sug;
                    self.selected = 0;
                    self.recompute_suggestions();
                }
            }
            Message::SelectSuggestion(i) => {
                if let Some(sug) = self.suggestions.get(i).cloned() {
                    self.input = sug;
                    self.selected = 0;
                    self.recompute_suggestions();
                }
            }
            Message::Tick => {
                if let Some(v) = &self.video {
                    if let Some(rgb) = v.take_frame() {
                        self.current_frame = Some(rgb_to_handle(&rgb));
                    }
                }
            }
            Message::Quit => return iced::exit(),
        }
        Task::none()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let mut subs = Vec::new();
        if let Screen::Sorting = self.screen {
            subs.push(iced::keyboard::on_key_press(|key, _mods| {
                use iced::keyboard::key::Named;
                use iced::keyboard::Key;
                match key {
                    Key::Named(Named::ArrowUp) => Some(Message::SuggestionUp),
                    Key::Named(Named::ArrowDown) => Some(Message::SuggestionDown),
                    Key::Named(Named::Tab) => Some(Message::AcceptSuggestion),
                    Key::Named(Named::ArrowLeft) => Some(Message::ArrowLeft),
                    Key::Named(Named::ArrowRight) => Some(Message::ArrowRight),
                    _ => None,
                }
            }));
            if matches!(self.preview, PreviewState::Video) {
                subs.push(
                    iced::time::every(std::time::Duration::from_millis(33)).map(|_| Message::Tick),
                );
            }
        }
        Subscription::batch(subs)
    }

    pub fn view(&self) -> Element<'_, Message> {
        match self.screen {
            Screen::Setup => self.setup_view(),
            Screen::Sorting => self.sorting_view(),
            Screen::Done => self.done_view(),
        }
    }

    // --- screens -----------------------------------------------------------

    fn setup_view(&self) -> Element<'_, Message> {
        let mut col = column![
            text("pifbip").size(34),
            text("Sort files into folders, with previews.").size(14),
            Space::with_height(12),
            labeled_folder(
                "Source folder",
                &self.origin_input,
                Message::OriginChanged,
                Message::BrowseOrigin
            ),
            labeled_folder(
                "Destination folder",
                &self.destination_input,
                Message::DestinationChanged,
                Message::BrowseDestination
            ),
            row![
                text("Scan depth").width(Length::Fixed(140.0)),
                text_input("0", &self.depth_input)
                    .on_input(Message::DepthChanged)
                    .width(Length::Fixed(80.0)),
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center),
        ]
        .spacing(12)
        .max_width(580);

        if let Some(err) = &self.setup_error {
            col = col.push(text(err.clone()).color(theme::DANGER));
        }

        col = col.push(
            button(text("Start sorting"))
                .on_press(Message::Start)
                .padding([8, 16]),
        );

        container(col)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .padding(24)
            .style(theme::root_panel)
            .into()
    }

    fn sorting_view(&self) -> Element<'_, Message> {
        let session = match &self.session {
            Some(s) => s,
            None => return self.setup_view(),
        };
        let p = session.progress();

        // File list with the current file highlighted.
        let mut list = column![].spacing(2);
        for (i, name) in session.file_names().into_iter().enumerate() {
            let label = text(name).size(13);
            let rowel: Element<Message> = if i == p.index {
                container(label)
                    .padding([2, 6])
                    .width(Length::Fill)
                    .style(theme::selected_row)
                    .into()
            } else {
                container(label).padding([2, 6]).width(Length::Fill).into()
            };
            list = list.push(rowel);
        }

        // Suggestions (fuzzy-ranked), selected one highlighted.
        let mut sugg = column![].spacing(2);
        for (i, name) in self.suggestions.iter().take(MAX_SUGGESTIONS).enumerate() {
            let style = if i == self.selected {
                button::primary
            } else {
                button::secondary
            };
            sugg = sugg.push(
                button(text(name.clone()).size(13))
                    .on_press(Message::SelectSuggestion(i))
                    .width(Length::Fill)
                    .padding([3, 6])
                    .style(style),
            );
        }

        let header = text(format!(
            "[{}/{}]  moved {}  skipped {}",
            (p.index + 1).min(p.total),
            p.total,
            p.moved,
            p.skipped
        ))
        .size(14);

        let controls = row![
            button(text("← Back"))
                .on_press(Message::GoBack)
                .padding([6, 10]),
            button(text("Skip →"))
                .on_press(Message::Skip)
                .padding([6, 10]),
            button(text("Move"))
                .on_press(Message::Confirm)
                .padding([6, 10])
                .style(button::success),
        ]
        .spacing(8);

        let left = container(
            column![
                header,
                container(scrollable(list))
                    .height(Length::FillPortion(3))
                    .width(Length::Fill),
                text_input("subfolder name (empty = skip)", &self.input)
                    .on_input(Message::InputChanged)
                    .on_submit(Message::Confirm)
                    .padding(8),
                container(scrollable(sugg)).height(Length::FillPortion(2)),
                controls,
                text(self.status.clone()).size(12).color(theme::PRIMARY),
            ]
            .spacing(8)
            .padding(10),
        )
        .width(Length::Fixed(360.0))
        .height(Length::Fill)
        .style(theme::surface_panel);

        let right = container(self.preview_view())
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(10)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .style(theme::surface_panel);

        container(row![left, right].spacing(10))
            .padding(10)
            .style(theme::root_panel)
            .into()
    }

    fn preview_view(&self) -> Element<'_, Message> {
        match &self.preview {
            PreviewState::Empty => text("No preview").into(),
            PreviewState::Image(path) | PreviewState::Pdf(path) => {
                image(image::Handle::from_path(path))
                    .content_fit(ContentFit::Contain)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .into()
            }
            PreviewState::Text(body) => {
                scrollable(text(body.clone()).font(Font::MONOSPACE).size(13)).into()
            }
            PreviewState::Video => match &self.current_frame {
                Some(handle) => image(handle.clone())
                    .content_fit(ContentFit::Contain)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .into(),
                None => text("Loading video…").into(),
            },
            PreviewState::Other(info) => text(info.clone()).into(),
        }
    }

    fn done_view(&self) -> Element<'_, Message> {
        let summary = self
            .session
            .as_ref()
            .map(|s| {
                let p = s.progress();
                format!(
                    "Moved {}, skipped {}, of {} files.",
                    p.moved, p.skipped, p.total
                )
            })
            .unwrap_or_default();

        container(
            column![
                text("All done!").size(30),
                text(summary).size(15),
                button(text("Quit"))
                    .on_press(Message::Quit)
                    .padding([8, 16]),
            ]
            .spacing(16)
            .align_x(iced::Alignment::Center),
        )
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .style(theme::root_panel)
        .into()
    }

    // --- actions -----------------------------------------------------------

    fn do_confirm(&mut self) {
        let trimmed = self.input.trim().to_string();
        let outcome = self.session.as_mut().map(|s| {
            if trimmed.is_empty() {
                s.skip()
            } else {
                s.move_to(&trimmed)
            }
        });
        if let Some(o) = outcome {
            self.status = describe_outcome(&o);
            self.refresh();
        }
    }

    fn do_skip(&mut self) {
        let outcome = self.session.as_mut().map(|s| s.skip());
        if let Some(o) = outcome {
            self.status = describe_outcome(&o);
            self.refresh();
        }
    }

    fn do_goback(&mut self) {
        let outcome = self.session.as_mut().map(|s| s.go_back());
        if let Some(o) = outcome {
            self.status = describe_outcome(&o);
            self.refresh();
        }
    }

    /// Recompute suggestions + preview and detect completion after navigation.
    fn refresh(&mut self) {
        self.input.clear();
        self.selected = 0;
        self.recompute_suggestions();
        self.load_preview();
        if let Some(s) = &self.session {
            if s.is_done() {
                self.screen = Screen::Done;
            }
        }
    }

    fn recompute_suggestions(&mut self) {
        self.suggestions = self
            .session
            .as_ref()
            .map(|s| s.subdir_suggestions(&self.input))
            .unwrap_or_default();
        if self.selected >= self.suggestions.len() {
            self.selected = 0;
        }
    }

    fn load_preview(&mut self) {
        self.cleanup_preview();
        self.current_frame = None;

        let Some(session) = &self.session else {
            self.preview = PreviewState::Empty;
            return;
        };
        let (Some(path), Some(kind)) = (session.current(), session.current_kind()) else {
            self.preview = PreviewState::Empty;
            return;
        };

        self.preview = match kind {
            FileKind::Image => PreviewState::Image(path),
            FileKind::Pdf => match render_pdf_page(&path) {
                Some(png) => PreviewState::Pdf(png),
                None => PreviewState::Other(describe_file(&path)),
            },
            FileKind::Text => PreviewState::Text(read_text(&path)),
            FileKind::Video => {
                self.video = Some(VideoStream::start(path));
                PreviewState::Video
            }
            FileKind::Other => PreviewState::Other(describe_file(&path)),
        };
    }

    /// Tear down resources tied to the currently-shown preview.
    fn cleanup_preview(&mut self) {
        if let PreviewState::Pdf(png) = &self.preview {
            let _ = std::fs::remove_file(png);
        }
        self.video = None; // Drop signals the decoder thread to stop.
    }

    fn build_session(&self) -> Result<SortSession, String> {
        let origin = validate_dir(&self.origin_input, "source")?;
        let destination = validate_dir(&self.destination_input, "destination")?;
        let depth: u16 = self.depth_input.trim().parse().unwrap_or(0);
        Ok(SortSession::new(&origin, destination, depth))
    }
}

impl Drop for App {
    fn drop(&mut self) {
        self.cleanup_preview();
    }
}

fn validate_dir(input: &str, label: &str) -> Result<PathBuf, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(format!("Please choose a {label} folder."));
    }
    let path = PathBuf::from(trimmed)
        .canonicalize()
        .map_err(|e| format!("{label} '{trimmed}': {e}"))?;
    if !path.is_dir() {
        return Err(format!("{label} '{trimmed}' is not a directory."));
    }
    Ok(path)
}

fn describe_outcome(o: &Outcome) -> String {
    match o {
        Outcome::Moved {
            subfolder,
            dest_name,
        } => format!("Moved → {subfolder}/{dest_name}"),
        Outcome::Skipped => "Skipped".to_string(),
        Outcome::Undone => "Undone".to_string(),
        Outcome::MoveError(e) => format!("Error {e}"),
        Outcome::AtStart => "Already at the first file".to_string(),
        Outcome::Done => String::new(),
    }
}

fn describe_file(path: &Path) -> String {
    let size = path.metadata().map(|m| m.len()).unwrap_or(0);
    let mime = mime_guess::from_path(path)
        .first()
        .map(|m| m.to_string())
        .unwrap_or_else(|| "unknown type".to_string());
    format!("Type: {mime}\nSize: {}", format_size(size))
}

fn read_text(path: &Path) -> String {
    use std::io::Read;
    match std::fs::File::open(path) {
        Ok(f) => {
            let mut buf = String::new();
            let _ = f.take(TEXT_PREVIEW_BYTES).read_to_string(&mut buf);
            buf
        }
        Err(e) => format!("(cannot read file: {e})"),
    }
}

fn rgb_to_handle(rgb: &[u8]) -> image::Handle {
    let mut rgba = Vec::with_capacity(rgb.len() / 3 * 4);
    for c in rgb.chunks_exact(3) {
        rgba.extend_from_slice(&[c[0], c[1], c[2], 255]);
    }
    image::Handle::from_rgba(VIDEO_W as u32, VIDEO_H as u32, rgba)
}

fn labeled_folder<'a>(
    label: &'a str,
    value: &'a str,
    on_input: impl Fn(String) -> Message + 'a,
    on_browse: Message,
) -> Element<'a, Message> {
    row![
        text(label).width(Length::Fixed(140.0)),
        text_input("full path…", value)
            .on_input(on_input)
            .width(Length::Fill),
        button(text("Browse…")).on_press(on_browse).padding([6, 10]),
    ]
    .spacing(8)
    .align_y(iced::Alignment::Center)
    .into()
}
