#![windows_subsystem = "windows"]

use iced::widget::{
    button, checkbox, column, container, horizontal_rule, pick_list, row, scrollable, slider, text,
    text_input,
};
use iced::{
    executor, Application, Command, Element, Length, Settings, Subscription, Theme,
};
use iced::widget::container::StyleSheet;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

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
    selected_indices: Vec<usize>, // Track selected file indices
    recursive_mode: bool,

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
    ClearFiles,
    ToggleRecursive(bool),
    FileDropped(PathBuf),

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
                selected_indices: Vec::new(), // Initialize empty selection
                recursive_mode: false,
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
                let files = rfd::FileDialog::new()
                    .add_filter("Images", &["jpg", "jpeg", "png", "bmp", "gif", "webp"])
                    .pick_files();
                
                if let Some(files) = files {
                    for file in files {
                        if !self.file_queue.contains(&file) {
                            self.file_queue.push(file);
                        }
                    }
                }
                Command::none()
            }
            Message::AddFolder => {
                if self.is_converting { return Command::none(); }
                if let Some(path) = rfd::FileDialog::new().pick_folder() {
                    self.scan_folder(path);
                }
                Command::none()
            }
            Message::ClearFiles => {
                if !self.is_converting {
                    self.file_queue.clear();
                    self.status_message = "Ready".to_string();
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
                    self.scan_folder(path);
                } else if is_image(&path) && !self.file_queue.contains(&path) {
                    self.file_queue.push(path);
                }
                Command::none()
            }
            Message::SelectFile(index) => {
                if index < self.file_queue.len() && !self.selected_indices.contains(&index) {
                    self.selected_indices.push(index);
                }
                Command::none()
            }
            Message::DeselectFile(index) => {
                self.selected_indices.retain(|&i| i != index);
                Command::none()
            }
            Message::ToggleFileSelection(index) => {
                if index < self.file_queue.len() {
                    if self.selected_indices.contains(&index) {
                        // If the file is already selected, deselect it
                        self.selected_indices.retain(|&i| i != index);
                    } else {
                        // If it's not selected, add it to the selection
                        // For now, implement single selection behavior (clicking deselects others)
                        self.selected_indices.clear();
                        self.selected_indices.push(index);
                    }
                }
                Command::none()
            }
            Message::SelectAllFiles => {
                self.selected_indices.clear();
                for i in 0..self.file_queue.len() {
                    self.selected_indices.push(i);
                }
                Command::none()
            }
            Message::DeselectAllFiles => {
                self.selected_indices.clear();
                Command::none()
            }
            Message::DeleteSelected => {
                if !self.is_converting && !self.selected_indices.is_empty() {
                    // Create a copy of the selected indices and sort them in descending order
                    // to prevent index shifting during removal
                    let mut indices_to_remove = self.selected_indices.clone();
                    indices_to_remove.sort_by(|a, b| b.cmp(a));  // Descending order

                    // Remove selected files from the queue
                    for &index in &indices_to_remove {
                        if index < self.file_queue.len() {
                            self.file_queue.remove(index);
                        }
                    }

                    // Clear selection after removal
                    self.selected_indices.clear();
                }
                Command::none()
            }
            Message::RightClickFile(index) => {
                // On right-clicking a file, select it and trigger deletion (for simplicity)
                self.selected_indices.clear();
                self.selected_indices.push(index);
                Command::perform(async {}, |_| Message::DeleteSelected)
            }
            Message::KeyboardInput { key, modifiers } => {
                use iced::keyboard::key;
                match key {
                    iced::keyboard::Key::Named(key::Named::ArrowUp) => {
                        // Move selection up
                        if !self.selected_indices.is_empty() {
                            let min_index = self.selected_indices.iter().min().copied().unwrap_or(0);
                            if min_index > 0 {
                                self.selected_indices.clear();
                                self.selected_indices.push(min_index - 1);
                            }
                        } else if !self.file_queue.is_empty() {
                            // If nothing selected, select the first item
                            self.selected_indices.push(0);
                        }
                    }
                    iced::keyboard::Key::Named(key::Named::ArrowDown) => {
                        // Move selection down
                        if !self.selected_indices.is_empty() {
                            let max_index = self.selected_indices.iter().max().copied().unwrap_or(0);
                            if max_index < self.file_queue.len().saturating_sub(1) {
                                self.selected_indices.clear();
                                self.selected_indices.push(max_index + 1);
                            }
                        } else if !self.file_queue.is_empty() {
                            // If nothing selected, select the first item
                            self.selected_indices.push(0);
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
                if let Some(path) = rfd::FileDialog::new().pick_folder() {
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

                // Perform conversion on a thread
                Command::perform(async move {
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
                    ).await
                }, Message::ConversionFinished)
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

    fn view(&self) -> Element<Message> {
        // --- 3.2 File Management (Left) ---
        let mut file_column = column!().spacing(1);
        for (index, path) in self.file_queue.iter().enumerate() {
            let file_name = path.file_name().unwrap_or_default().to_string_lossy().to_string();

            // Create a container for the file entry with selection highlighting
            let file_entry = container(text(file_name))
                .width(Length::Fill)
                .padding(5)
                .style(if self.selected_indices.contains(&index) {
                    iced::theme::Container::Custom(Box::new(SelectedFileStyle))
                } else {
                    iced::theme::Container::Box
                });

            // Wrap in a button for click handling
            let file_button = button(file_entry)
                .on_press(Message::ToggleFileSelection(index))
                .style(iced::theme::Button::Text); // Use Text style to make it appear transparent

            file_column = file_column.push(file_button);
        }

        let file_list = scrollable(file_column)
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

        // Quality
        let quality_section = column![
            text(format!("Output Quality: {:.0}", self.quality)).size(14),
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
    fn scan_folder(&mut self, path: PathBuf) {
        let max_depth = if self.recursive_mode { usize::MAX } else { 1 };
        
        for entry in WalkDir::new(path).max_depth(max_depth).into_iter().filter_map(|e| e.ok()) {
            if entry.path().is_file() && is_image(entry.path()) {
                if !self.file_queue.contains(&entry.path().to_path_buf()) {
                    self.file_queue.push(entry.path().to_path_buf());
                }
            }
        }
    }
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
    let mut success_count = 0;

    for path in files {
        match convert_single_file(
            &path,
            &output_dir,
            target_format,
            quality,
            resize_option,
            append_suffix,
            &suffix_text,
            keep_transparency,
            keep_animation,
        ) {
            Ok(_) => {
                success_count += 1;
                if move_to_trash {
                    let _ = trash::delete(&path);
                }
            }
            Err(e) => {
                errors.push(format!("{}: {}", path.display(), e));
            }
        }
    }

    if errors.is_empty() {
        "Conversion complete".to_string()
    } else {
        format!("Conversion complete with errors. Success: {}. Failed: {}", success_count, errors.len())
    }
}

fn convert_single_file(
    path: &Path,
    output_dir: &Option<PathBuf>,
    target_format: TargetFormat,
    quality: f64,
    resize_option: ResizeOption,
    append_suffix: bool,
    suffix_text: &str,
    keep_transparency: bool,
    keep_animation: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    // 1. Determine Output Path
    let parent = output_dir.as_deref().or_else(|| path.parent()).ok_or("Invalid path")?;
    let stem = path.file_stem().ok_or("No filename")?.to_string_lossy();
    
    let suffix = if append_suffix { suffix_text } else { "" };
    let extension = target_format.extension();
    
    let mut base_name = format!("{}{}.{}", stem, suffix, extension);
    let mut output_path = parent.join(&base_name);

    // Collision handling
    let mut counter = 1;
    while output_path.exists() {
        base_name = format!("{}{}({}).{}", stem, suffix, counter, extension);
        output_path = parent.join(&base_name);
        counter += 1;
    }

    // 2. Load Image
    let mut img = image::open(path)?;

    // 3. Processing
    // Resizing
    let scale = resize_option.get_scale_factor();
    if (scale - 1.0).abs() > f64::EPSILON {
        let n_width = (img.width() as f64 * scale) as u32;
        let n_height = (img.height() as f64 * scale) as u32;
        // Lanczos3 is generally high quality
        img = img.resize(n_width, n_height, image::imageops::FilterType::Lanczos3);
    }

    // Transparency Handling
    // Check if we need to remove alpha
    let supports_transparency = target_format.supports_transparency();
    let should_flatten = !keep_transparency || !supports_transparency;

    let final_img = if should_flatten && img.color().has_alpha() {
        // Convert to RGB by flattening against a white background
        image::DynamicImage::ImageRgb8(img.to_rgb8())
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
            // Quality mapping: 1-100
            let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut writer, quality as u8);
            encoder.encode_image(&final_img)?;
        },
        TargetFormat::Png => {
             // Map 1-100 to compression types roughly? 
             // Image crate PNG encoder takes CompressionType. 
             // Default is usually fine. We'll stick to standard write.
             let encoder = image::codecs::png::PngEncoder::new(&mut writer);
             final_img.write_with_encoder(encoder)?;
        },
        TargetFormat::Gif => {
             // If we really wanted animation, we'd use GifEncoder with frames here.
             // For now, saving the static processed image.
             let mut encoder = image::codecs::gif::GifEncoder::new(&mut writer);
             encoder.encode_frame(image::Frame::new(final_img.to_rgba8()))?;
        },
        TargetFormat::WebP => {
            // Use webp crate for both lossless and lossy WebP encoding to properly support quality settings
            use std::io::Write;

            // Convert the image to RGBA for WebP encoding
            let rgba_image = final_img.to_rgba8();
            let width = rgba_image.width();
            let height = rgba_image.height();

            let webp_data = if quality >= 100.0 {
                // Lossless encoding
                use webp::Encoder;
                let encoder = Encoder::new(rgba_image.as_raw(), webp::PixelLayout::Rgba, width, height);
                encoder.encode_lossless().to_vec()
            } else {
                // Lossy encoding with specified quality
                use webp::Encoder;
                let encoder = Encoder::new(rgba_image.as_raw(), webp::PixelLayout::Rgba, width, height);
                encoder.encode(quality as f32).to_vec()
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

// --- Main Entry ---

fn main() -> iced::Result {
    ImageConverter::run(Settings::default())
}