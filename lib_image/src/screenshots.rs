extern crate magick_rust;
use lib_core::{define_cli_error, CliError, CriticalError, IOError, Printer};
use magick_rust::{
    bindings::{DrawRoundRectangle, MagickBooleanType_MagickTrue},
    magick_wand_genesis, CompositeOperator, DrawingWand, FilterType, GravityType, MagickWand,
    PixelWand,
};
use std::{fs, path::Path, sync::Once};

define_cli_error!(ImageMagickError, "ImageMagick command failed.");
define_cli_error!(ImageProcessingError, "Error in image processing: {details}.", { details: &str });

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum TextAlign {
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Corner {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
enum Corners {
    All,
    Only(Corner),
    Except(Corner),
    Top,
    Bottom,
    Left,
    Right,
}

#[derive(Debug, Clone)]
struct BoundingBox {
    x: usize,
    y: usize,
    width: usize,
    height: usize,
}

// Initialize MagickWand only once.
static START: Once = Once::new();

fn initialize_magick() {
    START.call_once(|| {
        magick_wand_genesis();
    });
}

fn with_image<P>(
    printer: &Printer,
    input: P,
    output: P,
    op: impl FnOnce(&mut MagickWand) -> Result<(), CliError>,
) -> Result<(), CliError>
where
    P: AsRef<Path>,
{
    initialize_magick();
    let mut wand = MagickWand::new();
    wand.read_image(
        input
            .as_ref()
            .to_str()
            .ok_or_else(|| CriticalError::new("invalid input path"))?,
    )
    .into_cli_res()?;
    op(&mut wand)?;
    if let Some(parent) = output.as_ref().parent() {
        fs::create_dir_all(parent).map_err(|e| IOError::with_debug(&e))?;
    }
    wand.write_image(
        output
            .as_ref()
            .to_str()
            .ok_or_else(|| CriticalError::new("invalid output path"))?,
    )
    .into_cli_res()?;
    printer.info(&format!("Image saved to {}.", output.as_ref().display()));
    Ok(())
}

pub fn process_screenshot_basic<P>(
    printer: &Printer,
    input: P,
    output: P,
    app_bar_height: u32,
) -> Result<(), CliError>
where
    P: AsRef<Path>,
{
    with_image(printer, input, output, |wand| {
        wand.crop_image(
            wand.get_image_width(),
            wand.get_image_height() - app_bar_height as usize,
            0,
            app_bar_height as isize,
        )
        .into_cli_res()?;
        Ok(())
    })
}

pub fn process_screenshot_headline_text<P>(
    printer: &Printer,
    input: P,
    output: P,
    app_bar_height: u32,
    foreground_color_hex: &str,
    background_color_hex: &str,
    text: &str,
    align: TextAlign,
) -> Result<(), CliError>
where
    P: AsRef<Path>,
{
    with_image(printer, input, output, |wand| {
        let width = wand.get_image_width();
        let height = wand.get_image_height();

        wand.crop_image(
            width,
            height - app_bar_height as usize,
            0,
            app_bar_height as isize,
        )
        .into_cli_res()?;

        let fg_color = color(foreground_color_hex)?;
        let bg_color = color(background_color_hex)?;

        // Start result canvas.
        let mut result = MagickWand::new();
        result.new_image(width, height, &fg_color).into_cli_res()?;

        // Shrink screenshot for frame-in-frame.
        let scale_factor = 0.9;
        let inner_width = (width as f64 * scale_factor) as usize;
        let inner_height = (height as f64 * scale_factor) as usize;
        wand.resize_image(inner_width, inner_height, FilterType::Lanczos)
            .into_cli_res()?;

        // Apply a border with rounded corners.
        let border_thickness = 40;
        let border_radius = 20.0;
        let corners = match align {
            TextAlign::Left => Corners::Only(Corner::TopRight),
            TextAlign::Center => Corners::Top,
            TextAlign::Right => Corners::Only(Corner::TopLeft),
        };
        let (inner_width, inner_height) =
            add_rounded_border(wand, &bg_color, border_thickness, border_radius, corners)?;

        // Overlay image onto background.
        let push_down = 180;
        let overlay_x = match align {
            TextAlign::Left => -(border_thickness as isize),
            TextAlign::Center => (width as isize - inner_width as isize) / 2,
            TextAlign::Right => width as isize - inner_width as isize + border_thickness as isize,
        };
        let overlay_y =
            height as isize - inner_height as isize + border_thickness as isize + push_down;
        result
            .compose_images(&wand, CompositeOperator::Over, true, overlay_x, overlay_y)
            .into_cli_res()?;

        // Add headline text.
        let padding_top = 10;
        let padding_bottom = 30;
        let padding_left = 70;
        let padding_right = 70;
        let font_path = write_to_tmp(
            "Roboto-Light",
            include_bytes!("../res/Roboto/Roboto-Light.ttf"),
        )?;
        add_text(
            &mut result,
            text,
            format!("@{}", font_path.display()).as_str(),
            &bg_color,
            BoundingBox {
                x: padding_left,
                y: padding_top,
                width: width - padding_left - padding_right,
                height: overlay_y as usize - padding_top - padding_bottom,
            },
            match align {
                TextAlign::Left => TextAlign::Right,
                TextAlign::Center => TextAlign::Center,
                TextAlign::Right => TextAlign::Left,
            },
        )?;

        // Replace with final image.
        *wand = result.clone();
        Ok(())
    })
}

fn add_rounded_border(
    wand: &mut MagickWand,
    color: &PixelWand,
    thickness: usize,
    radius: f64,
    corners: Corners,
) -> Result<(usize, usize), CliError> {
    let width = wand.get_image_width();
    let height = wand.get_image_height();
    for corner in corners.iter() {
        round_corner(wand, corner, radius, radius * 4.0)?;
    }
    wand.border_image(color, thickness, thickness, CompositeOperator::Over)
        .into_cli_res()?;
    for corner in corners.iter() {
        round_corner(wand, corner, radius, radius * 4.0)?;
    }
    Ok((width + thickness * 2, height + thickness * 2))
}

fn round_corner(
    wand: &mut MagickWand,
    corner: Corner,
    radius: f64,
    mask_box_size: f64,
) -> Result<(), CliError> {
    let width = wand.get_image_width();
    let height = wand.get_image_height();

    // Draw a rounded rectangle mask.
    let mut corner_wand = DrawingWand::new();
    corner_wand.set_fill_color(&white());
    let mut mask = MagickWand::new();
    mask.new_image(width, height, &transparent())
        .into_cli_res()?;
    let (x, y) = match corner {
        Corner::TopLeft => (0.0, 0.0),
        Corner::TopRight => (width as f64 - mask_box_size, 0.0),
        Corner::BottomLeft => (0.0, height as f64 - mask_box_size),
        Corner::BottomRight => (width as f64 - mask_box_size, height as f64 - mask_box_size),
    };
    unsafe {
        // Not yet exposed in library, so use C-binding dircetly.
        DrawRoundRectangle(
            corner_wand.wand,
            x,
            y,
            x + mask_box_size - 1.0,
            y + mask_box_size - 1.0,
            radius,
            radius,
        );
    }
    mask.draw_image(&corner_wand).into_cli_res()?;

    // Add rest of image (excluding that corner) to mask.
    let mut fill_rest_wand = DrawingWand::new();
    fill_rest_wand.set_fill_color(&white());
    match corner {
        Corner::TopLeft => {
            fill_rest_wand.draw_rectangle(mask_box_size / 2.0, 0.0, width as f64, height as f64);
            fill_rest_wand.draw_rectangle(0.0, mask_box_size / 2.0, width as f64, height as f64);
        }
        Corner::TopRight => {
            fill_rest_wand.draw_rectangle(
                0.0,
                0.0,
                width as f64 - mask_box_size / 2.0,
                height as f64,
            );
            fill_rest_wand.draw_rectangle(0.0, mask_box_size / 2.0, width as f64, height as f64);
        }
        Corner::BottomLeft => {
            fill_rest_wand.draw_rectangle(mask_box_size / 2.0, 0.0, width as f64, height as f64);
            fill_rest_wand.draw_rectangle(
                0.0,
                0.0,
                width as f64,
                height as f64 - mask_box_size / 2.0,
            );
        }
        Corner::BottomRight => {
            fill_rest_wand.draw_rectangle(
                0.0,
                0.0,
                width as f64 - mask_box_size / 2.0,
                height as f64,
            );
            fill_rest_wand.draw_rectangle(
                0.0,
                0.0,
                width as f64,
                height as f64 - mask_box_size / 2.0,
            );
        }
    }
    mask.draw_image(&fill_rest_wand).into_cli_res()?;

    // Apply mask to the screenshot.
    wand.compose_images(&mask, CompositeOperator::DstIn, true, 0, 0)
        .into_cli_res()?;

    Ok(())
}

fn add_text(
    wand: &mut MagickWand,
    text: &str,
    font: &str,
    color: &PixelWand,
    fit_in_box: BoundingBox,
    align: TextAlign,
) -> Result<(), CliError> {
    let (wrapped, font_size) = wrap_or_scale_up(font, text, fit_in_box.width)?;

    let mut text_image = MagickWand::new();
    text_image
        .new_image(fit_in_box.width, fit_in_box.height, &transparent())
        .into_cli_res()?;

    let mut drawing_wand = DrawingWand::new();
    drawing_wand.set_font(font).into_cli_res()?;
    drawing_wand.set_text_antialias(MagickBooleanType_MagickTrue);
    drawing_wand.set_font_size(font_size);
    drawing_wand.set_fill_color(&color);
    drawing_wand.set_gravity(match align {
        TextAlign::Left => GravityType::West,
        TextAlign::Center => GravityType::Center,
        TextAlign::Right => GravityType::East,
    });
    text_image
        .annotate_image(&drawing_wand, 0.0, 0.0, 0.0, &wrapped)
        .into_cli_res()?;

    // Overlay text onto the screenshot.
    wand.compose_images(
        &text_image,
        CompositeOperator::Over,
        true,
        fit_in_box.x as isize,
        fit_in_box.y as isize,
    )
    .into_cli_res()?;
    Ok(())
}

fn wrap_or_scale_up(
    font: &str,
    text: &str,
    available_width: usize,
) -> Result<(String, f64), CliError> {
    let mut font_size = 140.0; // Start with a large initial size.
    let min_font_size = 80.0;

    // Try scaling the text down to fit within available width.
    while font_size > min_font_size {
        let text_width = get_text_width(font, text, font_size)?;
        if text_width <= available_width as f64 {
            // Found a size that fits, return scaled text and font size.
            return Ok((text.to_string(), font_size));
        }
        font_size -= 10.0; // Decrease font size incrementally.
    }

    // If we reach the minimum font size and it still doesn't fit, wrap the text.
    let font_size = min_font_size; // Use the minimum size for wrapped text.

    let mut wrapped_text = String::new();
    let mut current_line = String::new();

    for word in text.split_whitespace() {
        let test_line = if current_line.is_empty() {
            word.to_string()
        } else {
            format!("{} {}", current_line, word)
        };

        let line_width = get_text_width(font, &test_line, font_size)?;
        if line_width <= available_width as f64 {
            current_line = test_line;
        } else {
            if !wrapped_text.is_empty() {
                wrapped_text.push('\n');
            }
            wrapped_text.push_str(&current_line);
            current_line = word.to_string();
        }
    }

    if !current_line.is_empty() {
        if !wrapped_text.is_empty() {
            wrapped_text.push('\n');
        }
        wrapped_text.push_str(&current_line);
    }

    Ok((wrapped_text, font_size))
}

fn get_text_width(font: &str, text: &str, size: f64) -> Result<f64, CliError> {
    let mut test_img = MagickWand::new();
    test_img
        .new_image(size as usize * text.len(), size as usize * 3, &black())
        .into_cli_res()?;

    let mut drawing_wand = DrawingWand::new();
    drawing_wand.set_font(font).into_cli_res()?;
    drawing_wand.set_text_antialias(MagickBooleanType_MagickTrue);
    drawing_wand.set_font_size(size);
    drawing_wand.set_fill_color(&white());
    drawing_wand.set_gravity(GravityType::West);

    test_img
        .annotate_image(&drawing_wand, 0.0, 0.0, 0.0, text)
        .into_cli_res()?;

    let width_before = test_img.get_image_width();
    test_img.trim_image(0.0).into_cli_res()?;
    let width_after = test_img.get_image_width();

    if width_before == width_after {
        Err(ImageProcessingError::new(
            "(code issue) incorrect test image size was used, since trimmed image was not shorter than original"
        ))
    } else {
        Ok(width_after as f64)
    }
}

impl Corners {
    fn iter(&self) -> Vec<Corner> {
        match self {
            Corners::All => vec![
                Corner::TopLeft,
                Corner::TopRight,
                Corner::BottomLeft,
                Corner::BottomRight,
            ],
            Corners::Only(c) => vec![*c],
            Corners::Except(c) => vec![
                Corner::TopLeft,
                Corner::TopRight,
                Corner::BottomLeft,
                Corner::BottomRight,
            ]
            .into_iter()
            .filter(|&x| x != *c)
            .collect(),
            Corners::Top => vec![Corner::TopLeft, Corner::TopRight],
            Corners::Bottom => vec![Corner::BottomLeft, Corner::BottomRight],
            Corners::Left => vec![Corner::TopLeft, Corner::BottomLeft],
            Corners::Right => vec![Corner::TopRight, Corner::BottomRight],
        }
    }
}

fn write_to_tmp<C>(key: &str, bytes: C) -> Result<std::path::PathBuf, CliError>
where
    C: AsRef<[u8]>,
{
    let path = std::env::temp_dir().join(key);
    fs::write(&path, bytes).map_err(|e| IOError::with_debug(&e))?;
    Ok(path)
}

fn color(hex: &str) -> Result<PixelWand, CliError> {
    let mut c = PixelWand::new();
    c.set_color(hex).into_cli_res()?;
    Ok(c)
}

fn black() -> PixelWand {
    let mut c = PixelWand::new();
    c.set_color("#000000")
        .expect("Hard-coded color should be valid.");
    c
}

fn white() -> PixelWand {
    let mut c = PixelWand::new();
    c.set_color("#FFFFFF")
        .expect("Hard-coded color should be valid.");
    c
}

fn transparent() -> PixelWand {
    let mut c = PixelWand::new();
    c.set_color("#FFFFFF")
        .expect("Hard-coded color should be valid.");
    c
}

trait IntoCliError {
    fn into_cli_err(self) -> CliError;
}

impl IntoCliError for magick_rust::MagickError {
    fn into_cli_err(self) -> CliError {
        ImageMagickError::with_debug(&self)
    }
}

trait IntoCliResult<T> {
    fn into_cli_res(self) -> Result<T, CliError>;
}

impl<T> IntoCliResult<T> for Result<T, magick_rust::MagickError> {
    fn into_cli_res(self) -> Result<T, CliError> {
        self.map_err(|e| e.into_cli_err())
    }
}
