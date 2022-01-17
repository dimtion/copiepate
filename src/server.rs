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
    pub fn start(&mut self) -> Result<(), Error> {
        log::info!("Starting server {}", self.address);
        let listener = TcpListener::bind(self.address)?;

        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    handle_connection(stream, self.clipboard_ctx);
                }
                Err(e) => {
                    log::error!("Connection failed: {}", e);
                }
            }
        }

        Ok(())
    }
}

fn handle_connection<P, T>(stream: T, clipboard_ctx: &mut P)
where
    P: ClipboardProvider,
    T: Sized + Read,
{
    let mut reader = BufReader::new(stream);
    loop {
        let frame_response = match NetFrame::from_net(&mut reader) {
            Ok(frame) => frame,
            Err(e) => {
                log::error!("Error reading stream: {}", e);
                break;
            }
        };

        let frame = match frame_response {
            Some(f) => f,
            None => {
                log::trace!("End of stream");
                break;
            }
        };

        let content_string = String::from_utf8_lossy(&frame.content);
        log::debug!("Received message: '{}'", &content_string);
        match clipboard_ctx.set_contents(content_string.to_string()) {
            Ok(_) => {
                log::info!("New message saved to clipboard");
            }
            Err(e) => {
                log::error!("Failed to write to clipboard: {}", e);
            }
        }
    }
}
