use anyhow::{anyhow, Context, Result};
use base64::{engine::general_purpose::STANDARD_NO_PAD, Engine};
use std::{
    fs,
    io::{BufRead, BufReader, Write},
    net::Shutdown,
    os::unix::{
        net::{UnixListener, UnixStream},
        prelude::PermissionsExt,
    },
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
use tracing::{info, trace, warn};

const EOT: u8 = 4;

pub struct SocketServer {
    listener: UnixListener,
    path: PathBuf,
}

impl SocketServer {
    pub fn create(path: PathBuf, set_permissions: bool) -> Result<Self> {
        if let Some(run_dir) = path.parent() {
            fs::create_dir_all(run_dir)
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
        info!("Created at socket at '{}'", path.display());
        Ok(Self { listener, path })
    }

    pub fn handle<F>(&mut self, f: F) -> Result<()>
    where
        F: Fn(&[u8]) -> Option<Vec<u8>>,
    {
        if let Ok((mut stream, _)) = self.listener.accept() {
            let reader = std::io::BufReader::new(stream.try_clone()?);
            for msg in reader.split(EOT) {
                let msg = msg?;
                let decoded = STANDARD_NO_PAD.decode(&msg)?;
                trace!("Received message: {decoded:?}");
                if let Some(resp) = f(&decoded) {
                    trace!("Responding with: {resp:?}");
                    let encoded = STANDARD_NO_PAD.encode(&resp);
                    stream.write_all(&[encoded.as_bytes(), &[EOT]].concat())?;
                } else {
                    stream.write_all(&[EOT])?;
                }
                stream.flush()?;
            }
        }
        Ok(())
    }

    pub fn serve<F>(&mut self, f: F) -> Result<()>
    where
        F: Fn(&[u8]) -> Option<Vec<u8>>,
    {
        loop {
            self.handle(&f)?;
        }
    }

    pub fn serve_until<F>(&mut self, shutdown: Arc<AtomicBool>, f: F) -> Result<()>
    where
        F: Fn(&[u8]) -> Option<Vec<u8>>,
    {
        while !shutdown.load(Ordering::Relaxed) {
            self.handle(&f)?;
        }
        Ok(())
    }
}

impl Drop for SocketServer {
    fn drop(&mut self) {
        fs::remove_file(&self.path).unwrap();
    }
}

pub struct SocketClient {
    stream: UnixStream,
    reader: BufReader<UnixStream>,
}

impl SocketClient {
    pub fn connect(path: PathBuf) -> Result<Self> {
        let stream = UnixStream::connect(&path)
            .with_context(|| format!("Failed to connect to socket {path:?}"))?;
        let reader = BufReader::new(stream.try_clone()?);
        Ok(Self { stream, reader })
    }

    pub fn try_send(&mut self, msg: &[u8]) -> Result<Option<Vec<u8>>> {
        trace!("Sending message over socket: {msg:?}");
        let encoded = STANDARD_NO_PAD.encode(msg);
        self.stream.write_all(&[encoded.as_bytes(), &[EOT]].concat())?;
        self.stream.flush()?;
        let mut response = Vec::new();
        self.reader.read_until(EOT, &mut response)?;
        response.pop();
        if response.is_empty() {
            return Ok(None);
        }
        let decoded = STANDARD_NO_PAD.decode(&response)?;
        trace!("Received response: {decoded:?}");
        Ok(Some(decoded))
    }

    pub fn send(&mut self, msg: &[u8]) -> Result<Vec<u8>> {
        self.try_send(msg)?
            .ok_or_else(|| anyhow!("Empty response: server error"))
    }
}

impl Drop for SocketClient {
    fn drop(&mut self) {
        self.stream.shutdown(Shutdown::Write).unwrap();
    }
}
