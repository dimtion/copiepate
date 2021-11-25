use std::{
    io::{Error, Write},
    net::TcpStream,
};

use crate::NetFrame;

pub fn start_client(address: &str, message: &[u8]) -> Result<(), Error> {
    let mut stream = TcpStream::connect(address)?;

    let frame = NetFrame::from(message);
    log::debug!("Frame size: {}", frame.size);
    stream.write_all(&frame.to_net())?;

    // Empty frame means end of steam
    stream.write_all(&NetFrame::empty_frame().to_net())?;

    log::info!("Message sent");

    Ok(())
}
