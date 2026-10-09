# Image Converter

Simple desktop app for converting images between JPG, PNG, GIF, BMP, and WebP.
Supports animation for WebP and GIF conversion.

## Build

Requires Rust 1.88 or newer.

```sh
cargo run --release
```

## Linux Requirements

- glibc 2.43 or newer
- Wayland or X11

## Use

1. Add files or a folder, or drop them on the window. Turn on **Recursive Search** to include subfolders.
2. Choose a target format, output folder, quality, and resize.
3. Click **Start Conversion**.

The quality slider applies to JPG and WebP. WebP at 100 is lossless. PNG, GIF, and BMP ignore the quality slider.
