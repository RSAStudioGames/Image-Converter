# Image Converter

Desktop app for converting images between JPG, PNG, GIF, BMP, and WebP.

WebP and GIF

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

Files are written next to the source unless you set an output directory. The default name is the original stem plus `_converted`. If that name is already taken, a number is appended (`photo_converted(1).jpg`). Originals stay in place unless **Move original to Recycle Bin** is checked.

The quality slider applies to JPG and WebP. WebP at 100 is lossless. PNG, GIF, and BMP ignore the quality slider.
