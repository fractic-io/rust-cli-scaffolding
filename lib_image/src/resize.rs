use std::path::Path;

use lib_core::{CliError, Printer};
use magick_rust::{CompositeOperator, FilterType, MagickWand};

use crate::common::{black, with_image, IntoCliResult};

pub enum CropBehaviour {
    FitWidthClipBottom,
    FitWidthClipTop,
    FitHeightClipRight,
    FitHeightClipLeft,
}

pub fn resize_image<P>(
    pr: &Printer,
    input: P,
    output: P,
    target_width: usize,
    target_height: usize,
    crop_behaviour: CropBehaviour,
) -> Result<(), CliError>
where
    P: AsRef<Path>,
{
    with_image(pr, input, output, |wand| {
        let orig_w = wand.get_image_width();
        let orig_h = wand.get_image_height();

        // In case resize has borders, set background color to the top-left
        // pixel of source image.
        let bg_color = wand.get_image_pixel_color(0, 0).unwrap_or(black());

        let orig_aspect = orig_w as f64 / orig_h as f64;

        // Figure out new dimensions for the resized image.
        let (new_w, new_h) = match crop_behaviour {
            // For FitWidth..., we want the resized image’s width to match target_width:
            CropBehaviour::FitWidthClipBottom | CropBehaviour::FitWidthClipTop => {
                let new_w = target_width;
                let new_h = (new_w as f64 / orig_aspect).round() as usize;
                (new_w, new_h)
            }
            // For FitHeight..., we want the resized image’s height to match target_height:
            CropBehaviour::FitHeightClipRight | CropBehaviour::FitHeightClipLeft => {
                let new_h = target_height;
                let new_w = (new_h as f64 * orig_aspect).round() as usize;
                (new_w, new_h)
            }
        };

        // Resize image in-place to the new dimensions (preserving aspect ratio, using Lanczos).
        wand.resize_image(new_w, new_h, FilterType::Lanczos)
            .into_cli_res()?;

        // Now, we want a new blank canvas of the requested final size with our background color:
        let canvas = MagickWand::new();
        canvas
            .new_image(target_width, target_height, &bg_color)
            .into_cli_res()?;

        // Decide how to position the resized image onto that canvas.
        let (offset_x, offset_y) = match crop_behaviour {
            CropBehaviour::FitWidthClipBottom => {
                // The top of the resized image is at y=0; if new_h >
                // target_height, it gets clipped at the bottom.
                let x = ((target_width as isize) - (new_w as isize)) / 2; // center horizontally
                let y = 0;
                (x, y)
            }
            CropBehaviour::FitWidthClipTop => {
                // The bottom of the resized image is at y = target_height -
                // new_h; if new_h > target_height, it clips the top.
                let x = ((target_width as isize) - (new_w as isize)) / 2; // center horizontally
                let y = (target_height as isize) - (new_h as isize);
                (x, y)
            }
            CropBehaviour::FitHeightClipRight => {
                // The left of the resized image is at x=0; if new_w >
                // target_width, it gets clipped on the right.
                let x = 0;
                let y = ((target_height as isize) - (new_h as isize)) / 2; // center vertically
                (x, y)
            }
            CropBehaviour::FitHeightClipLeft => {
                // The right of the resized image is at x = target_width -
                // new_w; if new_w > target_width, it clips the left side.
                let x = (target_width as isize) - (new_w as isize);
                let y = ((target_height as isize) - (new_h as isize)) / 2; // center vertically
                (x, y)
            }
        };

        // Composite the resized “wand” (the original image we resized in-place)
        // onto the canvas.
        canvas
            .compose_images(&wand, CompositeOperator::Over, true, offset_x, offset_y)
            .into_cli_res()?;

        // Replace the original `wand` with our new “canvas” result.
        *wand = canvas;

        Ok(())
    })
}
