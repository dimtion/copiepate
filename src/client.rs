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
        let mut stream = TcpStream::connect(self.address)?;

        let frame = NetFrame::from(message);
        log::debug!("Frame size: {}", frame.size);
        stream.write_all(&frame.to_net())?;

        // Empty frame means end of steam
        stream.write_all(&NetFrame::empty_frame().to_net())?;

        log::info!("Message sent");

        Ok(())
    }
}
