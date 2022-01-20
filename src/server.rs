use std::{
    io::{BufReader, Error, Read},
    net::TcpListener,
};

use clipboard::ClipboardProvider;

use crate::NetFrame;

pub struct Server<'a, 'b, P>
where
    P: ClipboardProvider,
{
    pub address: &'a str,
    pub clipboard_ctx: &'b mut P,
}

impl<P: ClipboardProvider> Server<'_, '_, P> {
    /// Start Copiepate server. Listen for ever.
    pub fn start(&mut self) -> Result<(), Error> {
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

    fn handle_connection<T: Sized + Read>(&mut self, stream: T) {
        let mut reader = BufReader::new(stream);
        loop {
            let frame = match NetFrame::from_net(&mut reader) {
                Ok(frame) => frame,
                Err(e) => {
                    log::error!("Error reading stream: {}", e);
                    break;
                }
            };

            match frame.frame_type {
                crate::NetFrameType::Open => (), // TODO
                crate::NetFrameType::Message => self.handle_message(&frame),
                crate::NetFrameType::Close => {
                    log::trace!("Received end of stream");
                    break;
                },
            }
        }
    }

    fn handle_message(&mut self, frame: &NetFrame) {
        let content_string = String::from_utf8_lossy(&frame.payload);
        log::debug!("Received message: '{}'", &content_string);
        match self.clipboard_ctx.set_contents(content_string.to_string()) {
            Ok(_) => {
                log::info!("New message saved to clipboard");
            }
            Err(e) => {
                log::error!("Failed to write to clipboard: {}", e);
            }
        }
    }
}
