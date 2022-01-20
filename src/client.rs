use std::{
    io::{Error, Write},
    net::TcpStream,
};

use crate::NetFrame;

pub struct Client<'a> {
    pub address: &'a str,
}

impl Client<'_> {
    pub fn send_message(&self, message: &[u8]) -> Result<(), Error> {
        log::debug!("Sending message to {}", self.address);
        let mut stream = TcpStream::connect(self.address)?;

        let frame = NetFrame::from(message);
        log::trace!("Frame size: {}", frame.payload_size);
        stream.write_all(&frame.to_net())?;

        // Empty frame means end of steam
        stream.write_all(&NetFrame::close_frame().to_net())?;

        log::info!("Message sent successfully");

        Ok(())
    }
}
