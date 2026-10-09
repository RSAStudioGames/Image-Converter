#![windows_subsystem = "windows"]

use iced::widget::{
    button, checkbox, column, container, horizontal_rule, pick_list, row, scrollable, slider, text,
    text_input, Space,
};
use iced::{
    executor, Application, Command, Element, Length, Settings, Subscription, Theme,
};
use iced::widget::container::StyleSheet;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use walkdir::WalkDir;

const ROW_HEIGHT: f32 = 32.0;
const LIST_OVERSCAN: usize = 8;
const INITIAL_VIEWPORT_HEIGHT: f32 = 2000.0;

// --- Data Structures ---

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TargetFormat {
    Jpg,
    WebP,
    Png,
    Gif,
    Bmp,
}

impl std::fmt::Display for TargetFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                TargetFormat::Jpg => "JPG",
                TargetFormat::WebP => "WEBP",
                TargetFormat::Png => "PNG",
                TargetFormat::Gif => "GIF",
                TargetFormat::Bmp => "BMP",
            }
        )
    }
}

impl TargetFormat {
    const ALL: [TargetFormat; 5] = [
        TargetFormat::Jpg,
        TargetFormat::WebP,
        TargetFormat::Png,
        TargetFormat::Gif,
        TargetFormat::Bmp,
    ];

    fn extension(&self) -> &'static str {
        match self {
            TargetFormat::Jpg => "jpg",
            TargetFormat::WebP => "webp",
            TargetFormat::Png => "png",
            TargetFormat::Gif => "gif",
            TargetFormat::Bmp => "bmp",
        }
    }

    fn supports_transparency(&self) -> bool {
        matches!(self, TargetFormat::Png | TargetFormat::WebP | TargetFormat::Gif)
    }

    fn supports_animation(&self) -> bool {
        matches!(self, TargetFormat::Gif | TargetFormat::WebP | TargetFormat::Png)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ResizeOption {
    NoResize,
    Reduce25,
    Reduce50,
    Reduce75,
    Reduce100,
    Enlarge25,
    Enlarge50,
    Enlarge75,
    Enlarge100,
}

impl std::fmt::Display for ResizeOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                ResizeOption::NoResize => "No resize",
                ResizeOption::Reduce25 => "Reduce by 25%",
                ResizeOption::Reduce50 => "Reduce by 50%",
                ResizeOption::Reduce75 => "Reduce by 75%",
                ResizeOption::Reduce100 => "Reduce by 100%",
                ResizeOption::Enlarge25 => "Enlarge by 25%",
                ResizeOption::Enlarge50 => "Enlarge by 50%",
                ResizeOption::Enlarge75 => "Enlarge by 75%",
                ResizeOption::Enlarge100 => "Enlarge by 100%",
            }
        )
    }
}

impl ResizeOption {
    const ALL: [ResizeOption; 9] = [
        ResizeOption::NoResize,
        ResizeOption::Reduce25,
        ResizeOption::Reduce50,
        ResizeOption::Reduce75,
        ResizeOption::Reduce100,
        ResizeOption::Enlarge25,
        ResizeOption::Enlarge50,
        ResizeOption::Enlarge75,
        ResizeOption::Enlarge100,
    ];

    fn get_scale_factor(&self) -> f64 {
        match self {
            ResizeOption::NoResize => 1.0,
            // Linear interpolation between 1.0 and 0.5
            ResizeOption::Reduce25 => 0.875,
            ResizeOption::Reduce50 => 0.75,
            ResizeOption::Reduce75 => 0.625,
            ResizeOption::Reduce100 => 0.50,
            ResizeOption::Enlarge25 => 1.25,
            ResizeOption::Enlarge50 => 1.50,
            ResizeOption::Enlarge75 => 1.75,
            ResizeOption::Enlarge100 => 2.00,
        }
    }
}

// --- Application State ---

struct ImageConverter {
    // File Management
    file_queue: Vec<PathBuf>,
    queued_paths: HashSet<PathBuf>,
    selected_indices: HashSet<usize>,
    recursive_mode: bool,
    scan_generation: u64,
    file_list_id: scrollable::Id,
    list_scroll_y: f32,
    list_viewport_height: f32,

    // Settings
    target_format: TargetFormat,
    output_dir: Option<PathBuf>,
    quality: f64, // 1.0 to 100.0
    resize_option: ResizeOption,

    // Processing Options
    append_suffix: bool,
    suffix_text: String,
    move_to_trash: bool,
    remove_metadata: bool,
    keep_transparency: bool,
    keep_animation: bool,

    // State
    is_converting: bool,
    status_message: String,
}

#[derive(Debug, Clone)]
enum Message {
    // File Actions
    AddFiles,
    AddFolder,
    FilesPicked(Option<Vec<PathBuf>>),
    FolderPicked(Option<PathBuf>),
    FolderScanned { generation: u64, paths: Vec<PathBuf> },
    ClearFiles,
    ToggleRecursive(bool),
    FileDropped(PathBuf),
    FileListScrolled(scrollable::Viewport),

    // File Selection and Management
    SelectFile(usize),         // Index of file to select
    DeselectFile(usize),       // Index of file to deselect
    ToggleFileSelection(usize), // Toggle selection of a file
    SelectAllFiles,            // Select all files
    DeselectAllFiles,          // Deselect all files
    DeleteSelected,            // Delete all selected files
    RightClickFile(usize),     // Right click on a specific file
    KeyboardInput { key: iced::keyboard::Key, modifiers: iced::keyboard::Modifiers }, // Handle keyboard events with modifiers

    // Settings Updates
    SetTargetFormat(TargetFormat),
    SetOutputDirectory,
    OutputDirectoryPicked(Option<PathBuf>),
    UpdateQuality(f64),
    SetResizeOption(ResizeOption),
    ToggleAppendSuffix(bool),
    UpdateSuffixText(String),
    ToggleMoveToTrash(bool),
    ToggleRemoveMetadata(bool),
    ToggleKeepTransparency(bool),
    ToggleKeepAnimation(bool),

    // Conversion
    StartConversion,
    ConversionFinished(String), // Result message
}

impl Application for ImageConverter {
    type Executor = executor::Default;
    type Message = Message;
    type Theme = Theme;
    type Flags = ();

    fn new(_flags: ()) -> (Self, Command<Message>) {
        (
            ImageConverter {
                file_queue: Vec::new(),
                queued_paths: HashSet::new(),
                selected_indices: HashSet::new(),
                recursive_mode: false,
                scan_generation: 0,
                file_list_id: scrollable::Id::unique(),
                list_scroll_y: 0.0,
                list_viewport_height: INITIAL_VIEWPORT_HEIGHT,
                target_format: TargetFormat::Jpg,
                output_dir: None,
                quality: 95.0,
                resize_option: ResizeOption::NoResize,
                append_suffix: true,
                suffix_text: "_converted".to_string(),
                move_to_trash: false,
                remove_metadata: true,
                keep_transparency: true,
                keep_animation: true,
                is_converting: false,
                status_message: "Ready".to_string(),
            },
            Command::none(),
        )
    }

    fn title(&self) -> String {
        String::from("Image Converter")
    }

    fn update(&mut self, message: Message) -> Command<Message> {
        match message {
            Message::AddFiles => {
                if self.is_converting { return Command::none(); }
                Command::perform(
                    async {
                        rfd::AsyncFileDialog::new()
                            .add_filter("Images", &["jpg", "jpeg", "png", "bmp", "gif", "webp"])
                            .pick_files()
                            .await
                            .map(|files| files.into_iter().map(PathBuf::from).collect())
                    },
                    Message::FilesPicked,
                )
            }
            Message::AddFolder => {
                if self.is_converting { return Command::none(); }
                Command::perform(
                    async {
                        rfd::AsyncFileDialog::new()
                            .pick_folder()
                            .await
                            .map(PathBuf::from)
                    },
                    Message::FolderPicked,
                )
            }
            Message::FilesPicked(files) => {
                if self.is_converting { return Command::none(); }
                if let Some(files) = files {
                    for file in files {
                        self.push_file(file);
                    }
                }
                Command::none()
            }
            Message::FolderPicked(path) => {
                if self.is_converting { return Command::none(); }
                match path {
                    Some(path) => self.start_folder_scan(path),
                    None => Command::none(),
                }
            }
            Message::FolderScanned { generation, paths } => {
                if generation != self.scan_generation {
                    return Command::none();
                }
                for path in paths {
                    self.push_file(path);
                }
                if self.status_message == "Scanning folder..." {
                    self.status_message = "Ready".to_string();
                }
                Command::none()
            }
            Message::ClearFiles => {
                if !self.is_converting {
                    self.file_queue.clear();
                    self.queued_paths.clear();
                    self.selected_indices.clear();
                    self.scan_generation = self.scan_generation.wrapping_add(1);
                    self.status_message = "Ready".to_string();
                    return self.clamp_list_scroll();
                }
                Command::none()
            }
            Message::ToggleRecursive(val) => {
                self.recursive_mode = val;
                Command::none()
            }
            Message::FileDropped(path) => {
                if self.is_converting { return Command::none(); }
                if path.is_dir() {
                    return self.start_folder_scan(path);
                } else if is_image(&path) {
                    self.push_file(path);
                }
                Command::none()
            }
            Message::FileListScrolled(viewport) => {
                let offset = viewport.absolute_offset();
                self.list_scroll_y = offset.y.max(0.0);
                let height = viewport.bounds().height;
                if height > 1.0 {
                    self.list_viewport_height = height;
                }
                Command::none()
            }
            Message::SelectFile(index) => {
                if index < self.file_queue.len() {
                    self.selected_indices.insert(index);
                }
                Command::none()
            }
            Message::DeselectFile(index) => {
                self.selected_indices.remove(&index);
                Command::none()
            }
            Message::ToggleFileSelection(index) => {
                if index < self.file_queue.len() {
                    if self.selected_indices.contains(&index) {
                        self.selected_indices.remove(&index);
                    } else {
                        // Clicking selects only this file.
                        self.selected_indices.clear();
                        self.selected_indices.insert(index);
                    }
                }
                Command::none()
            }
            Message::SelectAllFiles => {
                self.selected_indices = (0..self.file_queue.len()).collect();
                Command::none()
            }
            Message::DeselectAllFiles => {
                self.selected_indices.clear();
                Command::none()
            }
            Message::DeleteSelected => {
                if !self.is_converting && !self.selected_indices.is_empty() {
                    // Remove from the end so earlier indices stay valid.
                    let mut indices_to_remove: Vec<usize> = self.selected_indices.iter().copied().collect();
                    indices_to_remove.sort_by(|a, b| b.cmp(a));

                    for index in indices_to_remove {
                        if index < self.file_queue.len() {
                            let path = self.file_queue.remove(index);
                            self.queued_paths.remove(&path);
                        }
                    }

                    self.selected_indices.clear();
                    return self.clamp_list_scroll();
                }
                Command::none()
            }
            Message::RightClickFile(index) => {
                // On right-clicking a file, select it and trigger deletion (for simplicity)
                self.selected_indices.clear();
                self.selected_indices.insert(index);
                Command::perform(async {}, |_| Message::DeleteSelected)
            }
            Message::KeyboardInput { key, modifiers } => {
                use iced::keyboard::key;
                match key {
                    iced::keyboard::Key::Named(key::Named::ArrowUp) => {
                        let next = if !self.selected_indices.is_empty() {
                            let min_index = self.selected_indices.iter().copied().min().unwrap_or(0);
                            if min_index > 0 {
                                self.selected_indices.clear();
                                self.selected_indices.insert(min_index - 1);
                                Some(min_index - 1)
                            } else {
                                None
                            }
                        } else if !self.file_queue.is_empty() {
                            self.selected_indices.insert(0);
                            Some(0)
                        } else {
                            None
                        };
                        if let Some(index) = next {
                            return self.reveal_index(index);
                        }
                    }
                    iced::keyboard::Key::Named(key::Named::ArrowDown) => {
                        let next = if !self.selected_indices.is_empty() {
                            let max_index = self.selected_indices.iter().copied().max().unwrap_or(0);
                            if max_index < self.file_queue.len().saturating_sub(1) {
                                self.selected_indices.clear();
                                self.selected_indices.insert(max_index + 1);
                                Some(max_index + 1)
                            } else {
                                None
                            }
                        } else if !self.file_queue.is_empty() {
                            self.selected_indices.insert(0);
                            Some(0)
                        } else {
                            None
                        };
                        if let Some(index) = next {
                            return self.reveal_index(index);
                        }
                    }
                    iced::keyboard::Key::Named(key::Named::Delete) => {
                        // Delete selected files
                        return Command::perform(async {}, |_| Message::DeleteSelected);
                    }
                    iced::keyboard::Key::Character(c) if (c == "a" || c == "A") && modifiers.control() => {
                        // Select all with Ctrl+A
                        return Command::perform(async {}, |_| Message::SelectAllFiles);
                    }
                    _ => {}
                }
                Command::none()
            }
            Message::SetTargetFormat(fmt) => {
                self.target_format = fmt;
                Command::none()
            }
            Message::SetOutputDirectory => {
                Command::perform(
                    async {
                        rfd::AsyncFileDialog::new()
                            .pick_folder()
                            .await
                            .map(PathBuf::from)
                    },
                    Message::OutputDirectoryPicked,
                )
            }
            Message::OutputDirectoryPicked(path) => {
                if let Some(path) = path {
                    self.output_dir = Some(path);
                }
                Command::none()
            }
            Message::UpdateQuality(val) => {
                self.quality = val;
                Command::none()
            }
            Message::SetResizeOption(opt) => {
                self.resize_option = opt;
                Command::none()
            }
            Message::ToggleAppendSuffix(val) => {
                self.append_suffix = val;
                Command::none()
            }
            Message::UpdateSuffixText(txt) => {
                self.suffix_text = txt;
                Command::none()
            }
            Message::ToggleMoveToTrash(val) => {
                self.move_to_trash = val;
                Command::none()
            }
            Message::ToggleRemoveMetadata(val) => {
                self.remove_metadata = val;
                Command::none()
            }
            Message::ToggleKeepTransparency(val) => {
                self.keep_transparency = val;
                Command::none()
            }
            Message::ToggleKeepAnimation(val) => {
                self.keep_animation = val;
                Command::none()
            }
            Message::StartConversion => {
                if self.file_queue.is_empty() {
                    self.status_message = "No files to convert.".to_string();
                    return Command::none();
                }

                self.is_converting = true;
                self.status_message = "Converting...".to_string();

                // Clone state needed for processing
                let files = self.file_queue.clone();
                let output_dir = self.output_dir.clone();
                let target_format = self.target_format;
                let quality = self.quality;
                let resize_option = self.resize_option;
                let append_suffix = self.append_suffix;
                let suffix_text = self.suffix_text.clone();
                let move_to_trash = self.move_to_trash;
                let keep_transparency = self.keep_transparency;
                let keep_animation = self.keep_animation;
                let _remove_metadata = self.remove_metadata; // Used in logic logic implicitly by image crate behavior

                // Each file runs on the blocking pool. The command future only
                // waits, so the GUI worker stays free to draw.
                Command::perform(
                    async move {
                        process_images(
                            files,
                            output_dir,
                            target_format,
                            quality,
                            resize_option,
                            append_suffix,
                            suffix_text,
                            move_to_trash,
                            keep_transparency,
                            keep_animation,
                        )
                        .await
                    },
                    Message::ConversionFinished,
                )
            }
            Message::ConversionFinished(msg) => {
                self.is_converting = false;
                self.status_message = msg;
                Command::none()
            }
        }
    }

    fn subscription(&self) -> Subscription<Message> {
        use iced::event::{listen_with};

        // Listen to events without returning the subscription immediately
        listen_with(|event, _status| {
            match event {
                iced::Event::Keyboard(iced::keyboard::Event::KeyPressed { key, modifiers, .. }) => {
                    Some(Message::KeyboardInput { key, modifiers })
                }
                iced::Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Right)) => {
                    // We'll handle right-click globally and determine what was clicked elsewhere.
                    // For now, just note that a right-click happened.
                    None  // Returning None means we don't send a message for this event
                }
                iced::Event::Window(_, iced::window::Event::FileDropped(path)) => {
                    Some(Message::FileDropped(path))
                }
                _ => None
            }
        })
    }

    fn view(&self) -> Element<'_, Message> {
        // --- 3.2 File Management (Left) ---
        let (start, end) = self.visible_range();
        let top_gap = start as f32 * ROW_HEIGHT;
        let bottom_gap = (self.file_queue.len() - end) as f32 * ROW_HEIGHT;

        let mut rows = column![].spacing(0);
        for index in start..end {
            let path = &self.file_queue[index];
            let file_name = path.file_name().unwrap_or_default().to_string_lossy().to_string();

            let file_entry = container(text(file_name).size(14))
                .width(Length::Fill)
                .height(Length::Fill)
                .padding([4, 5])
                .style(if self.selected_indices.contains(&index) {
                    iced::theme::Container::Custom(Box::new(SelectedFileStyle))
                } else {
                    iced::theme::Container::Box
                });

            let file_button = button(file_entry)
                .on_press(Message::ToggleFileSelection(index))
                .width(Length::Fill)
                .height(Length::Fixed(ROW_HEIGHT))
                .padding(0)
                .style(iced::theme::Button::Text);

            rows = rows.push(file_button);
        }

        let file_column = column![
            Space::with_height(Length::Fixed(top_gap)),
            rows,
            Space::with_height(Length::Fixed(bottom_gap)),
        ]
        .spacing(0);

        let file_list = scrollable(file_column)
            .id(self.file_list_id.clone())
            .on_scroll(Message::FileListScrolled)
            .height(Length::Fill)
            .width(Length::Fill);

        let file_controls = column![
            row![
                button("Add Files").on_press(Message::AddFiles),
                button("Add Folder").on_press(Message::AddFolder),
                button("Clear All").on_press(Message::ClearFiles).style(iced::theme::Button::Destructive),
                checkbox("Recursive Search", self.recursive_mode).on_toggle(Message::ToggleRecursive),
            ].spacing(10),
        ].spacing(10);

        let left_pane = container(column![
            text("File List").size(16),
            container(file_list).style(iced::theme::Container::Box).height(Length::Fill).padding(5),
            file_controls
        ].spacing(10)).width(Length::FillPortion(1)).padding(10);

        // --- 3.3 Conversion Settings (Right) ---
        
        // Output Dir
        let current_out_txt = self.output_dir.as_ref()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| "Same as Source".to_string());
        
        let output_section = column![
            text("Output Directory:").size(14),
            row![
                button("Set Output Directory").on_press(Message::SetOutputDirectory),
                text(current_out_txt).size(12)
            ].spacing(10).align_items(iced::Alignment::Center)
        ].spacing(5);

        // Quality. PNG is lossless, so the slider picks compression speed versus file size.
        let quality_label = if self.target_format == TargetFormat::Png {
            if self.quality >= 100.0 {
                format!("Output Quality: {:.0} (smaller)", self.quality)
            } else {
                format!("Output Quality: {:.0} (fast)", self.quality)
            }
        } else {
            format!("Output Quality: {:.0}", self.quality)
        };
        let quality_section = column![
            text(quality_label).size(14),
            slider(1.0..=100.0, self.quality, Message::UpdateQuality)
        ].spacing(5);

        // Resize
        let resize_section = column![
            text("Resize Options:").size(14),
            pick_list(&ResizeOption::ALL[..], Some(self.resize_option), Message::SetResizeOption)
        ].spacing(5);

        // Format
        let format_section = column![
            text("Target Format:").size(14),
            pick_list(&TargetFormat::ALL[..], Some(self.target_format), Message::SetTargetFormat)
        ].spacing(5);

        // Suffix & Options
        let suffix_row = row![
            checkbox("Append suffix", self.append_suffix).on_toggle(Message::ToggleAppendSuffix),
            text_input("Suffix", &self.suffix_text).on_input(Message::UpdateSuffixText).width(Length::Fixed(100.0)),
        ].spacing(10).align_items(iced::Alignment::Center);

        let options_section = column![
            suffix_row,
            checkbox("Move original to Recycle Bin", self.move_to_trash).on_toggle(Message::ToggleMoveToTrash),
            checkbox("Remove Metadata / EXIF", self.remove_metadata).on_toggle(Message::ToggleRemoveMetadata),
            checkbox("Keep Transparency", self.keep_transparency).on_toggle(Message::ToggleKeepTransparency),
            checkbox("Keep Animation", self.keep_animation).on_toggle(Message::ToggleKeepAnimation),
        ].spacing(10);

        let right_pane = container(column![
            text("Conversion Settings").size(20),
            format_section,
            horizontal_rule(10),
            output_section,
            horizontal_rule(10),
            quality_section,
            resize_section,
            horizontal_rule(10),
            options_section,
        ].spacing(15)).width(Length::FillPortion(1)).padding(10);

        // --- 3.4 Action (Bottom) ---
        let action_section = column![
            button(text("Start Conversion").size(24).horizontal_alignment(iced::alignment::Horizontal::Center))
                .padding(10)
                .width(Length::Fill)
                .style(iced::theme::Button::Primary)
                .on_press_maybe(if self.is_converting { None } else { Some(Message::StartConversion) }),
            text(&self.status_message).size(14)
        ].spacing(10).padding(10);

        // Main Layout
        column![
            row![left_pane, right_pane].spacing(20).height(Length::Fill),
            horizontal_rule(1),
            action_section
        ].into()
    }
}

impl ImageConverter {
    fn push_file(&mut self, path: PathBuf) {
        if self.queued_paths.insert(path.clone()) {
            self.file_queue.push(path);
        }
    }

    fn start_folder_scan(&mut self, path: PathBuf) -> Command<Message> {
        self.scan_generation = self.scan_generation.wrapping_add(1);
        let generation = self.scan_generation;
        let recursive = self.recursive_mode;
        let known = self.queued_paths.clone();
        self.status_message = "Scanning folder...".to_string();

        Command::perform(
            async move {
                let paths = tokio::task::spawn_blocking(move || {
                    scan_folder_paths(path, recursive, known)
                })
                .await
                .unwrap_or_default();
                (generation, paths)
            },
            |(generation, paths)| Message::FolderScanned { generation, paths },
        )
    }

    fn visible_range(&self) -> (usize, usize) {
        let len = self.file_queue.len();
        if len == 0 {
            return (0, 0);
        }

        let start = (self.clamped_scroll_y() / ROW_HEIGHT).floor() as usize;
        let start = start.saturating_sub(LIST_OVERSCAN).min(len);
        let viewport_rows = (self.list_viewport_height / ROW_HEIGHT).ceil() as usize + 1;
        let end = (start + viewport_rows + LIST_OVERSCAN * 2).min(len);
        (start, end)
    }

    fn reveal_index(&mut self, index: usize) -> Command<Message> {
        let len = self.file_queue.len();
        if len == 0 {
            return Command::none();
        }

        let start = ((self.clamped_scroll_y() / ROW_HEIGHT).floor() as usize).min(len);
        let rows = ((self.list_viewport_height / ROW_HEIGHT).ceil() as usize).max(1);
        let end = (start + rows).min(len);
        if index >= start && index < end {
            return Command::none();
        }

        let y = if index < start {
            index as f32 * ROW_HEIGHT
        } else {
            ((index as f32 + 1.0) * ROW_HEIGHT - self.list_viewport_height).max(0.0)
        };
        self.list_scroll_y = y;
        scrollable::scroll_to(
            self.file_list_id.clone(),
            scrollable::AbsoluteOffset { x: 0.0, y },
        )
    }

    fn max_scroll_y(&self) -> f32 {
        (self.file_queue.len() as f32 * ROW_HEIGHT - self.list_viewport_height).max(0.0)
    }

    fn clamped_scroll_y(&self) -> f32 {
        self.list_scroll_y.clamp(0.0, self.max_scroll_y())
    }

    fn clamp_list_scroll(&mut self) -> Command<Message> {
        let y = self.clamped_scroll_y();
        if (self.list_scroll_y - y).abs() < f32::EPSILON {
            return Command::none();
        }
        self.list_scroll_y = y;
        scrollable::scroll_to(
            self.file_list_id.clone(),
            scrollable::AbsoluteOffset { x: 0.0, y },
        )
    }
}

fn scan_folder_paths(path: PathBuf, recursive: bool, mut known: HashSet<PathBuf>) -> Vec<PathBuf> {
    let max_depth = if recursive { usize::MAX } else { 1 };
    let mut found = Vec::new();

    for entry in WalkDir::new(path).max_depth(max_depth).into_iter().filter_map(|e| e.ok()) {
        if entry.path().is_file() && is_image(entry.path()) {
            let file_path = entry.into_path();
            if known.insert(file_path.clone()) {
                found.push(file_path);
            }
        }
    }

    found
}

// --- Custom Styles ---

struct SelectedFileStyle;

impl StyleSheet for SelectedFileStyle {
    type Style = iced::Theme;

    fn appearance(&self, _style: &Self::Style) -> iced::widget::container::Appearance {
        iced::widget::container::Appearance {
            background: Some(iced::Background::Color([0.2, 0.4, 0.8].into())), // Blue background for selected items
            border: iced::Border {
                color: [0.1, 0.3, 0.7].into(),
                width: 1.0,
                radius: 4.0.into(), // Use the radius field in the border instead
            },
            ..Default::default() // Keep other properties as default
        }
    }
}

// --- Helpers & Logic ---

fn is_image(path: &Path) -> bool {
    if let Some(ext) = path.extension() {
        let ext_str = ext.to_string_lossy().to_lowercase();
        matches!(ext_str.as_str(), "jpg" | "jpeg" | "png" | "gif" | "bmp" | "webp")
    } else {
        false
    }
}

/// How many photos to decode at once.
/// A 24-megapixel RGBA buffer is about 96 MB, so more than four in flight
/// can push the process into swap. Stay at or under the CPU count as well.
fn batch_concurrency() -> usize {
    let cpus = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);
    batch_limit_for(cpus)
}

fn batch_limit_for(cpus: usize) -> usize {
    cpus.clamp(1, 4)
}

/// Pick a free output path and record it so later files in this batch cannot
/// claim the same name. Workers must not do this themselves: two of them can
/// both see a path as free and overwrite each other.
fn reserve_output_path(
    path: &Path,
    output_dir: &Option<PathBuf>,
    target_format: TargetFormat,
    append_suffix: bool,
    suffix_text: &str,
    reserved: &mut HashSet<PathBuf>,
) -> Result<PathBuf, &'static str> {
    let parent = output_dir
        .as_deref()
        .or_else(|| path.parent())
        .ok_or("Invalid path")?;
    let stem = path.file_stem().ok_or("No filename")?.to_string_lossy();
    let suffix = if append_suffix { suffix_text } else { "" };
    let extension = target_format.extension();

    let mut base_name = format!("{stem}{suffix}.{extension}");
    let mut output_path = parent.join(&base_name);
    let mut counter = 1;
    while output_path.exists() || reserved.contains(&output_path) {
        base_name = format!("{stem}{suffix}({counter}).{extension}");
        output_path = parent.join(&base_name);
        counter += 1;
    }
    reserved.insert(output_path.clone());
    Ok(output_path)
}

async fn process_images(
    files: Vec<PathBuf>,
    output_dir: Option<PathBuf>,
    target_format: TargetFormat,
    quality: f64,
    resize_option: ResizeOption,
    append_suffix: bool,
    suffix_text: String,
    move_to_trash: bool,
    keep_transparency: bool,
    keep_animation: bool,
) -> String {
    let mut errors = Vec::new();
    let mut reserved = HashSet::new();
    let mut jobs = Vec::with_capacity(files.len());

    for path in files {
        match reserve_output_path(
            &path,
            &output_dir,
            target_format,
            append_suffix,
            &suffix_text,
            &mut reserved,
        ) {
            Ok(output_path) => jobs.push((path, output_path)),
            Err(e) => errors.push(format!("{}: {e}", path.display())),
        }
    }

    let limit = batch_concurrency();
    let sem = Arc::new(tokio::sync::Semaphore::new(limit));
    let mut set = tokio::task::JoinSet::new();

    for (path, output_path) in jobs {
        let sem = Arc::clone(&sem);
        let label = path.display().to_string();
        set.spawn(async move {
            let _permit = sem.acquire_owned().await.map_err(|_| {
                format!("{label}: conversion pool closed")
            })?;
            tokio::task::spawn_blocking(move || {
                match convert_single_file(
                    &path,
                    &output_path,
                    target_format,
                    quality,
                    resize_option,
                    keep_transparency,
                    keep_animation,
                ) {
                    Ok(()) => {
                        if move_to_trash {
                            let _ = trash::delete(&path);
                        }
                        Ok(())
                    }
                    Err(e) => Err(format!("{}: {e}", path.display())),
                }
            })
            .await
            .unwrap_or_else(|err| Err(format!("{label}: {err}")))
        });
    }

    let mut success_count = 0;
    while let Some(joined) = set.join_next().await {
        match joined {
            Ok(Ok(())) => success_count += 1,
            Ok(Err(msg)) => errors.push(msg),
            Err(err) => errors.push(format!("conversion task failed: {err}")),
        }
    }

    if errors.is_empty() {
        "Conversion complete".to_string()
    } else {
        format!(
            "Conversion complete with errors. Success: {success_count}. Failed: {}",
            errors.len()
        )
    }
}

fn convert_single_file(
    path: &Path,
    output_path: &Path,
    target_format: TargetFormat,
    quality: f64,
    resize_option: ResizeOption,
    keep_transparency: bool,
    _keep_animation: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    // The output path was reserved before this worker started.
    // Shrinks use Catmull-Rom. Enlarges use Lanczos3 through fast_image_resize.
    // A JPEG at 1/2 or smaller is scaled in libjpeg-turbo during decode.
    let scale = resize_option.get_scale_factor();
    let img = load_scaled_image(path, scale)?;

    // 3. Processing

    // Transparency Handling
    // Check if we need to remove alpha
    let supports_transparency = target_format.supports_transparency();
    let should_flatten = !keep_transparency || !supports_transparency;

    let final_img = if should_flatten && img.color().has_alpha() {
        // Drop alpha. into_rgb8 does not composite onto white.
        image::DynamicImage::ImageRgb8(img.into_rgb8())
    } else {
        img
    };

    // Animation Handling Note:
    // The `image` crate's generic `save` method saves the buffer.
    // For GIFs, it usually saves the first frame unless using `GifEncoder` specifically with frames.
    // Implementing full animation resize/transparency pipeline is complex.
    // Logic below handles the "Keep Animation" unchecked state by default (DynamicImage is usually one frame).
    // If input was GIF and we want to keep animation, we'd need to iterate frames.
    
    // Simplification for the tool:
    // If Target is Static (JPG, BMP) -> `final_img.save` handles it (First frame).
    // If Target is Animated (GIF) AND Keep Animation AND Input is GIF -> We need special handling.
    // Due to complexity of resizing animated frames individually, this basic impl uses `save`
    // which effectively does "Condition B/C" (Static).
    // To truly support Condition A (Animated Resize), we would need to read frames, resize each, and encode.
    
    // 4. Saving
    let mut file = std::fs::File::create(&output_path)?;
    let mut writer = std::io::BufWriter::new(&mut file);

    match target_format {
        TargetFormat::Jpg => {
            encode_jpeg(final_img, quality, &mut writer)?;
        },
        TargetFormat::Png => {
             // Fast is the default. The quality slider is not a PNG compression control.
             let encoder = image::codecs::png::PngEncoder::new(&mut writer);
             final_img.write_with_encoder(encoder)?;
        },
        TargetFormat::Gif => {
             // Opaque frames stay RGB. encode() quantizes with from_rgb_speed at
             // this encoder's speed (1) and sets disposal to Background.
             let mut encoder = image::codecs::gif::GifEncoder::new(&mut writer);
             if final_img.color().has_alpha() {
                 encoder.encode_frame(image::Frame::new(final_img.into_rgba8()))?;
             } else {
                 let rgb = final_img.into_rgb8();
                 encoder.encode(
                     rgb.as_raw(),
                     rgb.width(),
                     rgb.height(),
                     image::ExtendedColorType::Rgb8,
                 )?;
             }
        },
        TargetFormat::WebP => {
            use std::io::Write;
            use webp::{Encoder, PixelLayout};

            let encode = |encoder: Encoder<'_>| -> Vec<u8> {
                if quality >= 100.0 {
                    encoder.encode_lossless().to_vec()
                } else {
                    encoder.encode(quality as f32).to_vec()
                }
            };

            // Opaque images stay 3 bytes per pixel. into_* moves an existing buffer.
            let webp_data = if final_img.color().has_alpha() {
                let rgba = final_img.into_rgba8();
                encode(Encoder::new(
                    rgba.as_raw(),
                    PixelLayout::Rgba,
                    rgba.width(),
                    rgba.height(),
                ))
            } else {
                let rgb = final_img.into_rgb8();
                encode(Encoder::new(
                    rgb.as_raw(),
                    PixelLayout::Rgb,
                    rgb.width(),
                    rgb.height(),
                ))
            };

            writer.write_all(&webp_data)?;
        }
        TargetFormat::Bmp => {
            let encoder = image::codecs::bmp::BmpEncoder::new(&mut writer);
            final_img.write_with_encoder(encoder)?;
        },
    }

    Ok(())
}

fn encode_jpeg<W: std::io::Write>(
    img: image::DynamicImage,
    quality: f64,
    writer: &mut W,
) -> Result<(), Box<dyn std::error::Error>> {
    let quality = quality.clamp(1.0, 100.0).round() as i32;
    // Quality 100 stays lossy. Grayscale stays one channel; color uses 4:2:0.
    // into_rgb8 reuses the buffer when the image is already RGB8.
    let encoded = if let image::DynamicImage::ImageLuma8(gray) = img {
        turbojpeg::compress(
            turbojpeg::Image {
                pixels: gray.as_raw().as_slice(),
                width: gray.width() as usize,
                pitch: gray.width() as usize,
                height: gray.height() as usize,
                format: turbojpeg::PixelFormat::GRAY,
            },
            quality,
            turbojpeg::Subsamp::Gray,
        )?
    } else {
        let rgb = img.into_rgb8();
        turbojpeg::compress(
            turbojpeg::Image {
                pixels: rgb.as_raw().as_slice(),
                width: rgb.width() as usize,
                pitch: rgb.width() as usize * 3,
                height: rgb.height() as usize,
                format: turbojpeg::PixelFormat::RGB,
            },
            quality,
            turbojpeg::Subsamp::Sub2x2,
        )?
    };
    writer.write_all(&encoded)?;
    Ok(())
}

const MAX_IMAGE_DIM: u32 = 32_768;
const MAX_DECODED_BYTES: u64 = 512 * 1024 * 1024;

fn reject_oversized_dimensions(width: u32, height: u32) -> Result<(), Box<dyn std::error::Error>> {
    if width > MAX_IMAGE_DIM || height > MAX_IMAGE_DIM {
        return Err(format!(
            "image is {width}x{height}, over the {MAX_IMAGE_DIM} pixel limit"
        )
        .into());
    }
    Ok(())
}

fn reject_oversized_buffer(bytes: usize) -> Result<(), Box<dyn std::error::Error>> {
    let bytes = u64::try_from(bytes).unwrap_or(u64::MAX);
    if bytes > MAX_DECODED_BYTES {
        return Err("decoded image exceeds the 512 MiB allocation limit".into());
    }
    Ok(())
}

fn is_jpeg_path(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| {
            let ext = ext.to_ascii_lowercase();
            ext == "jpg" || ext == "jpeg"
        })
        .unwrap_or(false)
}

fn target_size(width: u32, height: u32, scale: f64) -> (u32, u32) {
    let n_width = (width as f64 * scale) as u32;
    let n_height = (height as f64 * scale) as u32;
    (n_width.max(1), n_height.max(1))
}

/// Smallest libjpeg-turbo IDCT scale (1/8, 1/4, 1/2) that is still at least `scale`.
/// Scales above 1/2 return `None` so the caller decodes the full image.
fn jpeg_dct_factor(scale: f64) -> Option<turbojpeg::ScalingFactor> {
    if !(scale <= 0.5) {
        return None;
    }
    [
        turbojpeg::ScalingFactor::ONE_EIGHTH,
        turbojpeg::ScalingFactor::ONE_QUARTER,
        turbojpeg::ScalingFactor::ONE_HALF,
    ]
    .into_iter()
    .find(|factor| factor.num() as f64 / factor.denom() as f64 >= scale)
}

fn load_scaled_image(
    path: &Path,
    scale: f64,
) -> Result<image::DynamicImage, Box<dyn std::error::Error>> {
    if is_jpeg_path(path) {
        if scale <= 0.5 {
            if let Ok(img) = decode_jpeg_scaled(path, scale) {
                return Ok(img);
            }
        }
        if let Ok(img) = decode_jpeg(path) {
            if (scale - 1.0).abs() <= f64::EPSILON {
                return Ok(img);
            }
            return resize_to_scale(img, scale);
        }
    }
    if (scale - 1.0).abs() <= f64::EPSILON {
        return Ok(open_with_limits(path)?);
    }
    let img = open_with_limits(path)?;
    resize_to_scale(img, scale)
}

/// Strict width and height, plus the crate's default 512 MiB allocation cap.
/// `Limits` is non-exhaustive, so this starts from `Default` and only sets dimensions.
fn open_with_limits(path: &Path) -> image::ImageResult<image::DynamicImage> {
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_IMAGE_DIM);
    limits.max_image_height = Some(MAX_IMAGE_DIM);
    let mut reader = image::ImageReader::open(path)?;
    reader.limits(limits);
    reader.decode()
}

fn resize_to_scale(
    img: image::DynamicImage,
    scale: f64,
) -> Result<image::DynamicImage, Box<dyn std::error::Error>> {
    let (n_width, n_height) = target_size(img.width(), img.height(), scale);
    resize_dynamic(img, n_width, n_height, scale)
}

fn decode_jpeg(path: &Path) -> Result<image::DynamicImage, Box<dyn std::error::Error>> {
    let jpeg_data = std::fs::read(path)?;
    let mut decompressor = turbojpeg::Decompressor::new()?;
    let header = decompressor.read_header(&jpeg_data)?;
    let full_width = u32::try_from(header.width)?;
    let full_height = u32::try_from(header.height)?;
    reject_oversized_dimensions(full_width, full_height)?;
    let format = jpeg_pixel_format(header.colorspace)?;
    decompress_jpeg(&mut decompressor, &jpeg_data, header.width, header.height, format)
}

fn decode_jpeg_scaled(
    path: &Path,
    scale: f64,
) -> Result<image::DynamicImage, Box<dyn std::error::Error>> {
    let jpeg_data = std::fs::read(path)?;
    let mut decompressor = turbojpeg::Decompressor::new()?;
    let header = decompressor.read_header(&jpeg_data)?;
    if header.is_lossless {
        return Err("lossless JPEG has no IDCT scale".into());
    }
    let factor = jpeg_dct_factor(scale).ok_or("JPEG scale is larger than 1/2")?;
    let full_width = u32::try_from(header.width)?;
    let full_height = u32::try_from(header.height)?;
    reject_oversized_dimensions(full_width, full_height)?;
    let (target_width, target_height) = target_size(full_width, full_height, scale);

    decompressor.set_scaling_factor(factor)?;
    let scaled = header.scaled(factor);
    if scaled.width == 0 || scaled.height == 0 {
        return Err("JPEG IDCT scale produced an empty image".into());
    }

    let format = jpeg_pixel_format(header.colorspace)?;
    let decoded = decompress_jpeg(
        &mut decompressor,
        &jpeg_data,
        scaled.width,
        scaled.height,
        format,
    )?;
    let decoded_width = decoded.width();
    let decoded_height = decoded.height();

    if decoded_width == target_width && decoded_height == target_height {
        Ok(decoded)
    } else {
        resize_dynamic(decoded, target_width, target_height, scale)
    }
}

fn jpeg_pixel_format(
    colorspace: turbojpeg::Colorspace,
) -> Result<turbojpeg::PixelFormat, Box<dyn std::error::Error>> {
    // CMYK and YCCK can only be decoded as CMYK. Fall back to image::open for those.
    if matches!(
        colorspace,
        turbojpeg::Colorspace::CMYK | turbojpeg::Colorspace::YCCK
    ) {
        return Err("CMYK JPEG is decoded without libjpeg-turbo".into());
    }
    Ok(if colorspace == turbojpeg::Colorspace::Gray {
        turbojpeg::PixelFormat::GRAY
    } else {
        turbojpeg::PixelFormat::RGB
    })
}

fn decompress_jpeg(
    decompressor: &mut turbojpeg::Decompressor,
    jpeg_data: &[u8],
    width: usize,
    height: usize,
    format: turbojpeg::PixelFormat,
) -> Result<image::DynamicImage, Box<dyn std::error::Error>> {
    let channels = if format == turbojpeg::PixelFormat::GRAY {
        1
    } else {
        3
    };
    let pitch = width.checked_mul(channels).ok_or("JPEG pitch overflow")?;
    let buf_len = pitch.checked_mul(height).ok_or("JPEG buffer overflow")?;
    reject_oversized_buffer(buf_len)?;
    let mut pixels = vec![0u8; buf_len];
    let output = turbojpeg::Image {
        pixels: pixels.as_mut_slice(),
        width,
        pitch,
        height,
        format,
    };
    decompressor.decompress(jpeg_data, output)?;

    let decoded_width = u32::try_from(width)?;
    let decoded_height = u32::try_from(height)?;
    if channels == 1 {
        let buffer = image::GrayImage::from_raw(decoded_width, decoded_height, pixels)
            .ok_or("gray JPEG buffer size mismatch")?;
        Ok(image::DynamicImage::ImageLuma8(buffer))
    } else {
        let buffer = image::RgbImage::from_raw(decoded_width, decoded_height, pixels)
            .ok_or("RGB JPEG buffer size mismatch")?;
        Ok(image::DynamicImage::ImageRgb8(buffer))
    }
}

fn resize_dynamic(
    img: image::DynamicImage,
    dst_width: u32,
    dst_height: u32,
    scale: f64,
) -> Result<image::DynamicImage, Box<dyn std::error::Error>> {
    if img.width() == dst_width && img.height() == dst_height {
        return Ok(img);
    }
    let filter = if scale < 1.0 {
        fast_image_resize::FilterType::CatmullRom
    } else {
        fast_image_resize::FilterType::Lanczos3
    };
    Ok(match img {
        image::DynamicImage::ImageLuma8(buf) => image::DynamicImage::ImageLuma8(resize_u8_image(
            buf,
            dst_width,
            dst_height,
            fast_image_resize::PixelType::U8,
            filter,
        )?),
        image::DynamicImage::ImageLumaA8(buf) => {
            image::DynamicImage::ImageLumaA8(resize_u8_image(
                buf,
                dst_width,
                dst_height,
                fast_image_resize::PixelType::U8x2,
                filter,
            )?)
        }
        image::DynamicImage::ImageRgb8(buf) => image::DynamicImage::ImageRgb8(resize_u8_image(
            buf,
            dst_width,
            dst_height,
            fast_image_resize::PixelType::U8x3,
            filter,
        )?),
        image::DynamicImage::ImageRgba8(buf) => image::DynamicImage::ImageRgba8(resize_u8_image(
            buf,
            dst_width,
            dst_height,
            fast_image_resize::PixelType::U8x4,
            filter,
        )?),
        image::DynamicImage::ImageLuma16(buf) => {
            image::DynamicImage::ImageLuma16(resize_u16_image(
                buf,
                dst_width,
                dst_height,
                fast_image_resize::PixelType::U16,
                filter,
            )?)
        }
        image::DynamicImage::ImageLumaA16(buf) => {
            image::DynamicImage::ImageLumaA16(resize_u16_image(
                buf,
                dst_width,
                dst_height,
                fast_image_resize::PixelType::U16x2,
                filter,
            )?)
        }
        image::DynamicImage::ImageRgb16(buf) => image::DynamicImage::ImageRgb16(resize_u16_image(
            buf,
            dst_width,
            dst_height,
            fast_image_resize::PixelType::U16x3,
            filter,
        )?),
        image::DynamicImage::ImageRgba16(buf) => {
            image::DynamicImage::ImageRgba16(resize_u16_image(
                buf,
                dst_width,
                dst_height,
                fast_image_resize::PixelType::U16x4,
                filter,
            )?)
        }
        image::DynamicImage::ImageRgb32F(buf) => {
            image::DynamicImage::ImageRgb32F(resize_f32_image(
                buf,
                dst_width,
                dst_height,
                fast_image_resize::PixelType::F32x3,
                filter,
            )?)
        }
        image::DynamicImage::ImageRgba32F(buf) => {
            image::DynamicImage::ImageRgba32F(resize_f32_image(
                buf,
                dst_width,
                dst_height,
                fast_image_resize::PixelType::F32x4,
                filter,
            )?)
        }
        other => {
            let rgba = other.into_rgba8();
            image::DynamicImage::ImageRgba8(resize_u8_image(
                rgba,
                dst_width,
                dst_height,
                fast_image_resize::PixelType::U8x4,
                filter,
            )?)
        }
    })
}

fn resize_u8_image<P>(
    buf: image::ImageBuffer<P, Vec<u8>>,
    dst_width: u32,
    dst_height: u32,
    pixel_type: fast_image_resize::PixelType,
    filter: fast_image_resize::FilterType,
) -> Result<image::ImageBuffer<P, Vec<u8>>, Box<dyn std::error::Error>>
where
    P: image::Pixel<Subpixel = u8>,
{
    let (src_width, src_height) = (buf.width(), buf.height());
    let raw = resize_bytes(
        buf.into_raw(),
        src_width,
        src_height,
        dst_width,
        dst_height,
        pixel_type,
        filter,
    )?;
    image::ImageBuffer::from_raw(dst_width, dst_height, raw)
        .ok_or_else(|| "resized buffer size mismatch".into())
}

fn resize_u16_image<P>(
    buf: image::ImageBuffer<P, Vec<u16>>,
    dst_width: u32,
    dst_height: u32,
    pixel_type: fast_image_resize::PixelType,
    filter: fast_image_resize::FilterType,
) -> Result<image::ImageBuffer<P, Vec<u16>>, Box<dyn std::error::Error>>
where
    P: image::Pixel<Subpixel = u16>,
{
    let (src_width, src_height) = (buf.width(), buf.height());
    let raw = resize_bytes(
        u16s_to_bytes(buf.into_raw()),
        src_width,
        src_height,
        dst_width,
        dst_height,
        pixel_type,
        filter,
    )?;
    let pixels = bytes_to_u16s(raw)?;
    image::ImageBuffer::from_raw(dst_width, dst_height, pixels)
        .ok_or_else(|| "resized buffer size mismatch".into())
}

fn resize_f32_image<P>(
    buf: image::ImageBuffer<P, Vec<f32>>,
    dst_width: u32,
    dst_height: u32,
    pixel_type: fast_image_resize::PixelType,
    filter: fast_image_resize::FilterType,
) -> Result<image::ImageBuffer<P, Vec<f32>>, Box<dyn std::error::Error>>
where
    P: image::Pixel<Subpixel = f32>,
{
    let (src_width, src_height) = (buf.width(), buf.height());
    let raw = resize_bytes(
        f32s_to_bytes(buf.into_raw()),
        src_width,
        src_height,
        dst_width,
        dst_height,
        pixel_type,
        filter,
    )?;
    let pixels = bytes_to_f32s(raw)?;
    image::ImageBuffer::from_raw(dst_width, dst_height, pixels)
        .ok_or_else(|| "resized buffer size mismatch".into())
}

fn resize_bytes(
    src: Vec<u8>,
    src_width: u32,
    src_height: u32,
    dst_width: u32,
    dst_height: u32,
    pixel_type: fast_image_resize::PixelType,
    filter: fast_image_resize::FilterType,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let src_image = fast_image_resize::images::Image::from_vec_u8(
        src_width,
        src_height,
        src,
        pixel_type,
    )?;
    let mut dst_image =
        fast_image_resize::images::Image::new(dst_width, dst_height, pixel_type);
    let mut resizer = fast_image_resize::Resizer::new();
    let options = fast_image_resize::ResizeOptions::new()
        .resize_alg(fast_image_resize::ResizeAlg::Convolution(filter));
    resizer.resize(&src_image, &mut dst_image, &options)?;
    Ok(dst_image.into_vec())
}

fn u16s_to_bytes(values: Vec<u16>) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(values.len() * 2);
    for value in values {
        bytes.extend_from_slice(&value.to_ne_bytes());
    }
    bytes
}

fn bytes_to_u16s(bytes: Vec<u8>) -> Result<Vec<u16>, &'static str> {
    if bytes.len() % 2 != 0 {
        return Err("resized buffer size mismatch");
    }
    Ok(bytes
        .chunks_exact(2)
        .map(|chunk| u16::from_ne_bytes([chunk[0], chunk[1]]))
        .collect())
}

fn f32s_to_bytes(values: Vec<f32>) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(values.len() * 4);
    for value in values {
        bytes.extend_from_slice(&value.to_ne_bytes());
    }
    bytes
}

fn bytes_to_f32s(bytes: Vec<u8>) -> Result<Vec<f32>, &'static str> {
    if bytes.len() % 4 != 0 {
        return Err("resized buffer size mismatch");
    }
    Ok(bytes
        .chunks_exact(4)
        .map(|chunk| f32::from_ne_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect())
}

// --- Main Entry ---

fn main() -> iced::Result {
    ImageConverter::run(Settings::default())
}

#[cfg(test)]
mod tests {
    use super::{jpeg_dct_factor, resize_to_scale};

    #[test]
    fn jpeg_dct_factor_picks_smallest_sufficient_idct() {
        assert_eq!(
            jpeg_dct_factor(0.5),
            Some(turbojpeg::ScalingFactor::ONE_HALF)
        );
        assert_eq!(
            jpeg_dct_factor(0.3),
            Some(turbojpeg::ScalingFactor::ONE_HALF)
        );
        assert_eq!(
            jpeg_dct_factor(0.25),
            Some(turbojpeg::ScalingFactor::ONE_QUARTER)
        );
        assert_eq!(
            jpeg_dct_factor(0.2),
            Some(turbojpeg::ScalingFactor::ONE_QUARTER)
        );
        assert_eq!(
            jpeg_dct_factor(0.1),
            Some(turbojpeg::ScalingFactor::ONE_EIGHTH)
        );
        assert_eq!(jpeg_dct_factor(0.51), None);
        assert_eq!(jpeg_dct_factor(1.0), None);
        assert_eq!(jpeg_dct_factor(2.0), None);
    }

    #[test]
    fn shrink_rgb_stays_rgb_at_truncated_size() {
        let even = image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(
            100,
            80,
            image::Rgb([10, 20, 30]),
        ));
        let even_out = resize_to_scale(even, 0.5).unwrap();
        assert_eq!((even_out.width(), even_out.height()), (50, 40));
        assert!(matches!(even_out, image::DynamicImage::ImageRgb8(_)));

        let odd = image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(
            101,
            81,
            image::Rgb([10, 20, 30]),
        ));
        let odd_out = resize_to_scale(odd, 0.5).unwrap();
        assert_eq!((odd_out.width(), odd_out.height()), (50, 40));
        assert!(matches!(odd_out, image::DynamicImage::ImageRgb8(_)));
    }

    #[test]
    fn oversized_png_header_is_rejected() {
        let dir = std::env::temp_dir();
        let path = dir.join(format!(
            "image-converter-oversized-{}.png",
            std::process::id()
        ));
        std::fs::write(&path, png_with_dimensions(super::MAX_IMAGE_DIM + 1, 1)).unwrap();
        let error = super::open_with_limits(&path).unwrap_err();
        let _ = std::fs::remove_file(&path);
        assert!(
            matches!(error, image::ImageError::Limits(_)),
            "expected a decode limit error, got {error}"
        );
    }

    fn png_with_dimensions(width: u32, height: u32) -> Vec<u8> {
        let mut ihdr = Vec::with_capacity(13);
        ihdr.extend_from_slice(&width.to_be_bytes());
        ihdr.extend_from_slice(&height.to_be_bytes());
        ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);

        let mut png = vec![0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n'];
        png.extend(png_chunk(b"IHDR", &ihdr));
        png
    }

    fn png_chunk(kind: &[u8; 4], data: &[u8]) -> Vec<u8> {
        let mut signed = Vec::with_capacity(4 + data.len());
        signed.extend_from_slice(kind);
        signed.extend_from_slice(data);

        let mut chunk = Vec::with_capacity(12 + data.len());
        chunk.extend_from_slice(&(u32::try_from(data.len()).unwrap()).to_be_bytes());
        chunk.extend_from_slice(&signed);
        chunk.extend_from_slice(&png_crc(&signed).to_be_bytes());
        chunk
    }

    #[test]
    fn batch_limit_stays_between_one_and_four() {
        assert_eq!(super::batch_limit_for(0), 1);
        assert_eq!(super::batch_limit_for(1), 1);
        assert_eq!(super::batch_limit_for(2), 2);
        assert_eq!(super::batch_limit_for(3), 3);
        assert_eq!(super::batch_limit_for(8), 4);
    }

    #[test]
    fn reserved_names_skip_disk_and_earlier_files() {
        let dir = std::env::temp_dir().join(format!(
            "image-converter-reserve-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let photo_jpg = dir.join("photo.jpg");
        let photo_png = dir.join("photo.png");
        std::fs::write(&photo_jpg, b"a").unwrap();
        std::fs::write(&photo_png, b"b").unwrap();
        std::fs::write(dir.join("photo_converted.jpg"), b"taken").unwrap();

        let mut reserved = std::collections::HashSet::new();
        let first = super::reserve_output_path(
            &photo_jpg,
            &Some(dir.clone()),
            super::TargetFormat::Jpg,
            true,
            "_converted",
            &mut reserved,
        )
        .unwrap();
        let second = super::reserve_output_path(
            &photo_png,
            &Some(dir.clone()),
            super::TargetFormat::Jpg,
            true,
            "_converted",
            &mut reserved,
        )
        .unwrap();

        assert_eq!(first, dir.join("photo_converted(1).jpg"));
        assert_eq!(second, dir.join("photo_converted(2).jpg"));
        assert_ne!(first, second);
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn png_crc(data: &[u8]) -> u32 {
        let mut crc = 0xffff_ffffu32;
        for &byte in data {
            crc ^= u32::from(byte);
            for _ in 0..8 {
                let mask = (crc & 1).wrapping_neg();
                crc = (crc >> 1) ^ (0xedb8_8320 & mask);
            }
        }
        !crc
    }
}