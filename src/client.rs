use std::{
    io::{Read, Write},
    net::TcpStream,
};

use chacha20poly1305::aead::{Aead};
use chacha20poly1305::Key;
use chacha20poly1305::KeyInit;
use thiserror::Error;

use crate::{Cipher, NetFrame, Nonce, CLOSE_PAYLOAD};

pub struct Client<'a> {
    pub address: &'a str,
    cipher: Cipher,
    state: crate::ConnectionState,
}

#[derive(Error, Debug)]
pub enum ClientError {
    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error("Error parsing message")]
    ParsingError,

    #[error("Invalid state {0}")]
    InvalidState(String),

    #[error("Decryption error: {0}")]
    Decryption(chacha20poly1305::aead::Error),

    #[error("Encryption error: {0}")]
    Encryption(chacha20poly1305::aead::Error),
}

// TODO: handle multi parsing: encrypted vs non encrytped frames
// TODO: create a real state machine that disallow invalid state transisions at compile time.
impl<'a> Client<'a> {
    pub fn new(address: &'a str, key: &[u8]) -> Self {
        let key = Key::from_slice(key).to_owned();
        let cipher = Cipher::new(&key);
        Self {
            address,
            cipher,
            state: crate::ConnectionState::New,
        }
    }

    pub fn send(&mut self, message: &[u8]) -> Result<(), ClientError> {
        log::debug!("Sending message to {}", self.address);
        let mut stream = TcpStream::connect(self.address)?;

        log::trace!("Sending opening Frame");
        stream.write_all(&NetFrame::open_frame().to_net())?;

        self.handle_open(&self.next_frame(&mut stream)?)?;
        log::trace!("Received open response");

        self.send_message(&mut stream, message)?;

        log::trace!("Sending closing frame");
        self.send_close(&mut stream)?;

        stream.flush()?;

        Ok(())
    }

    fn next_frame<Stream: Sized + Read + Write>(
        &self,
        stream: &mut Stream,
    ) -> Result<NetFrame, ClientError> {
        Ok(NetFrame::from_net(stream)?)
    }

    fn handle_open(&mut self, frame: &NetFrame) -> Result<(), ClientError> {
        match self.state {
            crate::ConnectionState::New => (),
            _ => {
                return Err(ClientError::InvalidState(String::from(
                    "Invalid state while opening connection",
                )))
            }
        }

        let nonce: Nonce = frame
            .payload
            .clone()
            .try_into()
            .map_err(|_| ClientError::ParsingError)?;

        self.state = crate::ConnectionState::Opened(nonce);
        Ok(())
    }

    fn send_close<T: Write>(&mut self, stream: &mut T) -> Result<(), ClientError> {
        let nonce = match self.state {
            crate::ConnectionState::Opened(n) => n,
            _ => {
                return Err(ClientError::InvalidState(String::from(
                    "Invalid state while sending closing frame",
                )))
            }
        };
        let cipher_payload = self
            .cipher
            .encrypt(nonce.cipher_nonce(), CLOSE_PAYLOAD.as_slice())
            .map_err(ClientError::Encryption)?;
        stream.write_all(&NetFrame::close_frame(cipher_payload).to_net())?;
        self.state = crate::ConnectionState::Closed;
        Ok(())
    }

    fn send_message<T: Write>(
        &mut self,
        stream: &mut T,
        message: &[u8],
    ) -> Result<(), ClientError> {
        let nonce = match self.state {
            crate::ConnectionState::Opened(n) => n,
            _ => {
                return Err(ClientError::InvalidState(String::from(
                    "Invalid state while sending message",
                )))
            }
        };

        let cipher_message = self
            .cipher
            .encrypt(nonce.cipher_nonce(), message)
            .map_err(ClientError::Encryption)?;
        let message_frame = NetFrame::from(cipher_message);
        log::trace!("Sending payload with size: {}", message_frame.frame_size);
        stream.write_all(&message_frame.to_net())?;
        self.state = crate::ConnectionState::Opened(nonce.consume());
        Ok(())
    }
}
