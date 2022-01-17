use log::{error, trace};
use std::{io::{Error, Read}};

pub mod client;
pub mod server;

#[derive(Debug)]
struct NetFrame {
    size: u64,
    content: Vec<u8>,
}

impl NetFrame {
    fn to_net(&self) -> Vec<u8> {
        let mut vector = Vec::with_capacity(self.size as usize);
        vector.extend_from_slice(&self.size.to_le_bytes());
        vector.extend_from_slice(&self.content);
        vector
    }

    fn from_net<T: Read>(reader: &mut T) -> Result<Option<Self>, Error> {
        let mut size_buffer: [u8; 8] = [0; 8];
        match reader.read_exact(&mut size_buffer) {
            Ok(_) => (),
            Err(e) => {
                error!("End of stream while reading header");
                return Err(e);
            }
        }

        let frame_size = u64::from_le_bytes(size_buffer);
        trace!("NetFrame size: {}", frame_size);
        // Empty frame is end of stream
        if frame_size == 0 {
            return Ok(None);
        }

        let mut content = vec![0; frame_size as usize]; // 64bits only
        reader.read_exact(&mut content)?;

        Ok(Some(NetFrame {
            size: frame_size,
            content,
        }))
    }

    /// Empty frame should be the last frame sent
    fn empty_frame() -> NetFrame {
        Self {
            size: 0,
            content: Vec::with_capacity(0),
        }
    }
}

impl From<&[u8]> for NetFrame {
    fn from(message: &[u8]) -> Self {
        NetFrame {
            size: message.len() as u64,
            content: message.to_vec(),
        }
    }
}
