use anyhow::{Context, Result};
use std::{
    fs,
    os::unix::{net::UnixListener, prelude::PermissionsExt},
    path::PathBuf,
};
use tracing::{info, warn};

pub struct Socket {
    pub listener: UnixListener,
    pub path: PathBuf,
}

impl Socket {
    pub fn create(path: PathBuf, set_permissions: bool) -> Result<Socket> {
        if let Some(run_dir) = path.parent() {
            fs::create_dir_all(&run_dir)
                .with_context(|| format!("Failed to create runtime directory {run_dir:?}"))?;
        }
        if path.exists() {
            warn!("Removing exsisting socket '{}'", path.display());
            fs::remove_file(&path).with_context(|| "Failed to remove existing socket")?;
        }
        let listener = UnixListener::bind(&path)
            .with_context(|| format!("Failed to bind socket at {path:?}"))?;
        if set_permissions {
            // Set Unix permissions so that all users can write to the socket
            fs::set_permissions(&path, fs::Permissions::from_mode(0o722)).unwrap();
        }
        info!("Started socket at '{}'", path.display());
        Ok(Socket { listener, path })
    }
}

impl Drop for Socket {
    fn drop(&mut self) {
        fs::remove_file(&self.path).unwrap();
    }
}
