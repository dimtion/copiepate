use std::{
    io::{Read, Write},
    net::TcpListener,
    process::{Command, Stdio},
};

use chacha20poly1305::Key;
use chacha20poly1305::KeyInit;
use clipboard::ClipboardProvider;
use derive_builder::Builder;

use crate::Cipher;

use self::{
    connection::{Connection, Event, ExecEvent, PasteEvent},
    error::ServerError,
};

mod connection;
mod error;

/// Copiepate server.
#[derive(Builder)]
#[builder(pattern = "owned")]
pub struct Server<'a, 'b, P>
where
    P: ClipboardProvider,
{
    address: &'a str,
    clipboard_ctx: &'b mut P,

    #[builder(setter(name = "key", custom = true))]
    cipher: Cipher,

    #[builder(setter(into), default)]
    exec_command: Option<String>,
}

impl<'a, 'b, P> ServerBuilder<'a, 'b, P>
where
    P: ClipboardProvider,
{
    pub fn key(mut self, value: &[u8]) -> Self {
        let key = Key::from_slice(value).to_owned();
        let cipher = Cipher::new(&key);
        self.cipher = Some(cipher);
        self
    }
}

impl<'a, 'b, P> Server<'a, 'b, P>
where
    P: ClipboardProvider,
{
    /// Start Copiepate server. Listen for ever.
    pub fn start(&mut self) -> Result<(), ServerError> {
        log::info!("Starting server {}", self.address);
        let listener = TcpListener::bind(self.address)?;

        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    self.handle_connection(stream);
                }
                Err(e) => {
                    log::error!("Connection failed: {}", e);
                }
            }
        }

        Ok(())
    }

    fn handle_connection<Stream>(&mut self, stream: Stream)
    where
        Stream: Sized + Read + Write,
    {
        let connection = Connection::new(stream, self.cipher.clone());
        for paste_event in connection {
            match paste_event {
                Ok(Event::PasteEvent(e)) => self.handle_paste_event(&e),
                Ok(Event::ExecEvent(e)) => self.handle_exec_event(&e),
                Err(e) => {
                    log::error!("Error handling connection: {e}");
                    break;
                }
            }
        }
    }

    fn handle_paste_event(&mut self, event: &PasteEvent) {
        if let Err(e) = self.clipboard_ctx.set_contents(event.payload.clone()) {
            log::error!("Failed to write to clipboard: {}", e);
            return;
        }

        log::info!("New message saved to clipboard");
        if let Err(e) = self.exec_command(&event.payload) {
            log::error!("Failed to execute custom command: {}", e);
        };
    }

    fn handle_exec_event(&mut self, event: &ExecEvent) {
        log::info!("New message saved to clipboard");
        if let Err(e) = self.exec_command(&event.payload) {
            log::error!("Failed to execute custom command: {}", e);
        };
    }

    fn exec_command(&self, payload: &str) -> Result<(), ServerError> {
        let exec_command = match &self.exec_command {
            None => return Ok(()),
            Some(c) => c,
        };

        log::debug!("Executing command: {}", exec_command);
        let mut child = Command::new("sh")
            .arg("-c")
            .arg(exec_command)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        let mut child_stdin = child.stdin.take().expect("Failed to take child stdin");

        let payload = payload.to_owned();
        std::thread::spawn(move || {
            child_stdin
                .write_all(payload.as_bytes())
                .expect("Failed to write to stdin");
            child_stdin.flush().expect("Failed to flush stdin");
        });

        let output = child.wait_with_output()?;
        std::io::stdout().write_all(&output.stdout)?;
        std::io::stderr().write_all(&output.stderr)?;

        std::io::stdout().flush()?;
        std::io::stderr().flush()?;

        // Empty stderr line to have a separation between stdout message and service messages
        eprintln!();
        Ok(())
    }
}
