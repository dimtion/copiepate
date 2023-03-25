use std::io::{Read, Write};

use chacha20poly1305::aead::Aead;

use crate::{Cipher, NetFrame, Nonce, CLOSE_PAYLOAD};

use super::error::ServerError;

enum FrameEvent {
    Open,
    Message(PasteEvent),
    Exec(ExecEvent),
    Closed,
}

#[derive(Debug, Clone)]
pub struct PasteEvent {
    pub payload: String,
}

#[derive(Debug, Clone)]
pub struct ExecEvent {
    pub payload: String,
}

#[derive(Debug, Clone)]
pub enum Event {
    PasteEvent(PasteEvent),
    ExecEvent(ExecEvent),
}

pub struct Connection<Stream>
where
    Stream: Sized + Read + Write,
{
    stream: Stream,
    cipher: Cipher,
    state: crate::ConnectionState,
}

impl<Stream> Connection<Stream>
where
    Stream: Sized + Read + Write,
{
    pub fn new(stream: Stream, cipher: Cipher) -> Self {
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
            crate::NetFrameType::CopyMessage => self.handle_copy_message(&frame),
            crate::NetFrameType::ExecMessage => self.handle_exec_message(&frame),
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
            .map_err(ServerError::Decryption)?;

        if message != CLOSE_PAYLOAD {
            log::error!("Received invalid close payload. Discarding.");
            return Err(ServerError::InvalidState);
        }
        Ok(FrameEvent::Closed)
    }

    fn handle_open(&mut self, _frame: &NetFrame) -> Result<FrameEvent, ServerError> {
        log::trace!("Received open connection");
        // TODO: create state machine/other to make sure only one nounce is sent
        let nounce = Nonce::default();
        let nounce_frame = NetFrame::nounce_frame(&nounce);
        self.stream.write_all(&nounce_frame.to_net())?;
        self.state = crate::ConnectionState::Opened(nounce);
        Ok(FrameEvent::Open)
    }

    fn handle_copy_message(&mut self, frame: &NetFrame) -> Result<FrameEvent, ServerError> {
        log::trace!("Received new copy message");
        let payload = self.parse_message(frame)?;

        log::debug!("Received message: '{}'", &payload);
        Ok(FrameEvent::Message(PasteEvent { payload }))
    }

    fn handle_exec_message(&mut self, frame: &NetFrame) -> Result<FrameEvent, ServerError> {
        log::trace!("Received new event message");
        let payload = self.parse_message(frame)?;

        log::debug!("Received message: '{}'", &payload);
        Ok(FrameEvent::Exec(ExecEvent { payload }))
    }

    fn parse_message(&mut self, frame: &NetFrame) -> Result<String, ServerError> {
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
            .map_err(ServerError::Decryption)?;
        self.state = crate::ConnectionState::Opened(nounce.consume());

        // Using lossy conversion here in case copy event from the other system is not utf-8.
        // A better implementation would perhaps be passing the encoding in the protocol
        // Are there cases where we might paste non-string message?
        let content_string = String::from_utf8_lossy(&message);
        Ok(content_string.into_owned())
    }
}

impl<Stream> Iterator for Connection<Stream>
where
    Stream: Sized + Read + Write,
{
    type Item = Result<Event, ServerError>;

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
                FrameEvent::Message(m) => return Some(Ok(Event::PasteEvent(m))),
                FrameEvent::Exec(m) => return Some(Ok(Event::ExecEvent(m))),
            }
        }
    }
}
