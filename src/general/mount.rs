use crate::{CliError, Executor, IOMode};

pub async fn umount(ex: &Executor, path: &str, sudo_fallback: bool) -> Result<(), CliError> {
    match ex.execute("umount", &[path], IOMode::StreamOutput).await {
        Ok(_) => Ok(()),
        Err(e) => {
            let msg = e.message().to_lowercase();

            // If the target was not mounted, consider the operation a success.
            if msg.contains("not mounted") || msg.contains("not currently mounted") {
                return Ok(());
            }

            // When permission is denied and the caller opted-in, retry with sudo.
            if sudo_fallback
                && (msg.contains("permission denied") || msg.contains("operation not permitted"))
            {
                ex.execute("sudo", &["umount", path], IOMode::StreamOutput)
                    .await?;
                return Ok(());
            }

            Err(e)
        }
    }
}
