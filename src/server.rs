use std::{
    io::{BufReader, Error, Read},
    net::TcpListener,
};

use clipboard::{ClipboardContext, ClipboardProvider};

use crate::NetFrame;

fn handle_connection<T: Sized + Read>(stream: T, clipboard_ctx: &mut ClipboardContext) {
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
                log::debug!("Success writing to clipboard");
            }
            Err(e) => {
                log::error!("Failed to write to clipboard: {}", e);
            }
        }
    }
}

pub fn start_server(address: &str) -> Result<(), Error> {
    log::info!("Starting server {}", address);
    let mut clipboard_ctx: ClipboardContext = ClipboardProvider::new().unwrap();
    let listener = TcpListener::bind(address)?;

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                handle_connection(stream, &mut clipboard_ctx);
            }
            Err(e) => {
                log::error!("Connection failed: {}", e);
            }
        }
    }

    Ok(())
}
