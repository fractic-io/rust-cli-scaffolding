use crate::{CliError, Executor, IOMode};

pub fn umount(ex: &Executor, path: &str, sudo_fallback: bool) -> Result<(), CliError> {
    ex.execute("umount", &[path], IOMode::StreamOutput)
        .or_else(|e| {
            let msg = e.message().to_lowercase();

            // If the target was not mounted, consider the operation a success.
            if msg.contains("not mounted") || msg.contains("not currently mounted") {
                return Ok(String::new());
            }

            // When permission is denied and the caller opted-in, retry with sudo.
            if sudo_fallback
                && (msg.contains("permission denied") || msg.contains("operation not permitted"))
            {
                return ex
                    .execute("sudo", &["umount", path], IOMode::StreamOutput)
                    .map_err(|err| err);
            }

            Err(e)
        })
        .map(|_| ())
}
