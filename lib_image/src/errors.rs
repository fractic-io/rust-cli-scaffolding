use lib_core::define_cli_error;

define_cli_error!(ImageMagickError, "ImageMagick command failed.");
define_cli_error!(ImageProcessingError, "Error in image processing: {details}.", { details: &str });
