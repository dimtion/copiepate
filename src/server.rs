use std::{
    io::{Read, Write},
    net::TcpListener,
};

use chacha20poly1305::aead::{Aead, NewAead};
use chacha20poly1305::Key;
use clipboard::ClipboardProvider;
use thiserror::Error;

use crate::{Cipher, NetFrame, Nonce, CLOSE_PAYLOAD};

pub struct Server<'a, 'b, P>
where
    P: ClipboardProvider,
{
    address: &'a str,
    clipboard_ctx: &'b mut P,
    cipher: Cipher,
}
struct Connection<Stream>
where
    Stream: Sized + Read + Write,
{
    stream: Stream,
    cipher: Cipher,
    state: crate::ConnectionState,
}

#[derive(Error, Debug)]
pub enum ServerError {
    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error("Invalid state")]
    InvalidState,

    #[error("Decryption error: {0}")]
    Decryption(chacha20poly1305::aead::Error),
}

impl<Stream> Connection<Stream>
where
    Stream: Sized + Read + Write,
{
    fn new(stream: Stream, cipher: Cipher) -> Self {
        Self {
            stream,
            cipher,
            state: crate::ConnectionState::New,
        }
    }

    fn next_frame(&mut self) -> Result<FrameEvent, ServerError> {
        let frame = NetFrame::from_net(&mut self.stream)?;

        match frame.frame_type {
            crate::NetFrameType::Open => self.handle_open(&frame),
            crate::NetFrameType::Message => self.handle_message(&frame),
            crate::NetFrameType::Close => self.handle_close(&frame),
        }
    }

    fn handle_close(&self, frame: &NetFrame) -> Result<FrameEvent, ServerError> {
        log::trace!("Received end of stream");
        let nounce = match &self.state {
            crate::ConnectionState::Opened(nounce) => nounce,
            s => {
                log::error!("Invalid state '{s:?}' while handling closing message");
                return Err(ServerError::InvalidState);
            }
        };
        let message = self
            .cipher
            .decrypt(nounce.cipher_nonce(), frame.payload.as_ref())
            .map_err(|e| ServerError::Decryption(e))?;

        if message != CLOSE_PAYLOAD {
            log::error!("Received invalid close payload. Discarding.");
            return Err(ServerError::InvalidState);
        }
        Ok(FrameEvent::Closed)
    }

    fn handle_open(&mut self, _frame: &NetFrame) -> Result<FrameEvent, ServerError> {
        log::trace!("Received open connection");
        // TODO: create state machine/other to make sure only one nounce is sent
        let nounce = Nonce::new();
        let nounce_frame = NetFrame::nounce_frame(&nounce);
        self.stream.write_all(&nounce_frame.to_net())?;
        self.state = crate::ConnectionState::Opened(nounce);
        Ok(FrameEvent::Open)
    }

    fn handle_message(&mut self, frame: &NetFrame) -> Result<FrameEvent, ServerError> {
        log::trace!("Received new message");
        // TODO: Solve issue for frame_type leaking issue (parse if opened, otherwise decrypt?)
        let nounce = match &self.state {
            crate::ConnectionState::Opened(nounce) => nounce,
            s => {
                log::error!("Invalid state '{s:?}' while handling new message");
                return Err(ServerError::InvalidState);
            }
        };
        let message = self
            .cipher
            .decrypt(nounce.cipher_nonce(), frame.payload.as_ref())
            .map_err(|e| ServerError::Decryption(e))?;

        // Using lossy conversion here in case copy event from the other system is not utf-8.
        // A better implementation would perhaps be passing the encoding in the protocol
        // Are there cases where we might paste non-string message?
        let content_string = String::from_utf8_lossy(&message);

        log::debug!("Received message: '{}'", &content_string);
        self.state = crate::ConnectionState::Opened(nounce.consume());
        Ok(FrameEvent::Message(PasteEvent {
            payload: content_string.into_owned(),
        }))
    }
}

enum FrameEvent {
    Open,
    Message(PasteEvent),
    Closed,
}

#[derive(Debug, Clone)]
struct PasteEvent {
    pub payload: String,
}

impl<Stream> Iterator for Connection<Stream>
where
    Stream: Sized + Read + Write,
{
    type Item = Result<PasteEvent, ServerError>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let frame_event = self.next_frame();
            let frame_event = match frame_event {
                Ok(e) => e,
                Err(err) => return Some(Err(err)),
            };

            match frame_event {
                FrameEvent::Closed => return None,
                FrameEvent::Open => (), // Wait for next frame on Open
                FrameEvent::Message(m) => return Some(Ok(m)),
            }
        }
    }
}

impl<'a, 'b, P: ClipboardProvider> Server<'a, 'b, P> {
    pub fn new(address: &'a str, clipboard_ctx: &'b mut P, key: &[u8]) -> Self {
        let key = Key::from_slice(key).to_owned();
        let cipher = Cipher::new(&key);
        Self {
            address,
            clipboard_ctx,
            cipher,
        }
    }

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
                Ok(e) => self.handle_paste_event(&e),
                Err(e) => {
                    log::error!("Error handling connection: {e}");
                    break;
                }
            }
        }
    }

    fn handle_paste_event(&mut self, event: &PasteEvent) {
        match self.clipboard_ctx.set_contents(event.payload.clone()) {
            Ok(_) => {
                log::info!("New message saved to clipboard");
            }
            Err(e) => {
                log::error!("Failed to write to clipboard: {}", e);
            }
        }
    }
}
