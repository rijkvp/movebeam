use anyhow::{bail, Context, Result};
use std::{
    fs,
    io::{BufRead, BufReader, Write},
    net::Shutdown,
    os::unix::{
        net::{UnixListener, UnixStream},
        prelude::PermissionsExt,
    },
    path::PathBuf,
};
use tracing::{info, warn};

const EOT: u8 = 4;

pub struct SocketServer {
    listener: UnixListener,
    path: PathBuf,
}

impl SocketServer {
    pub fn create(path: PathBuf, set_permissions: bool) -> Result<Self> {
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
        Ok(Self { listener, path })
    }

    pub fn serve<F>(&mut self, handle: F) -> Result<()>
    where
        F: Fn(&[u8]) -> Option<Vec<u8>>,
    {
        // TODO: Allow sockets to shutdown
        loop {
            if let Ok((mut stream, _)) = self.listener.accept() {
                let reader = std::io::BufReader::new(stream.try_clone()?);
                for msg in reader.split(EOT) {
                    let msg = msg?;
                    if let Some(mut resp) = handle(&msg) {
                        println!("sending: {:?}", resp);
                        resp.push(EOT);
                        stream.write(&resp)?;
                    } else {
                        stream.write(&[EOT])?;
                    }
                    stream.flush()?;
                }
            }
        }
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
        self.stream.write(&[msg, &[EOT]].concat())?;
        self.stream.flush()?;
        let mut response = Vec::new();
        self.reader.read_until(EOT, &mut response)?;
        response.pop();
        if response.len() == 0 {
            return Ok(None);
        }
        Ok(Some(response))
    }

    pub fn send(&mut self, msg: &[u8]) -> Result<Vec<u8>> {
        match self.try_send(msg) {
            Ok(o) => {
                if let Some(v) = o {
                    Ok(v)
                } else {
                    bail!("Empty response: error on server.")
                }
            }
            Err(e) => Err(e),
        }
    }
}

impl Drop for SocketClient {
    fn drop(&mut self) {
        self.stream.shutdown(Shutdown::Write).unwrap();
    }
}
