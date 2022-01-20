use log::{error, trace};
use num_derive::{FromPrimitive, ToPrimitive};
use std::io::{Error, ErrorKind, Read};

pub mod client;
pub mod server;

// Protocol (current):
// client -- Message[payload] --> server
// client -- Message[payload] --> server
// client ------- Close[] ------> server

// Protocol (wanted):
// client ----------- Open[] -----------> server
// client <-------- Open[Nounce] -------- server
// client ------ Message[payload] ------> server [Encrypted with Nounce]
// client ------ Message[payload] ------> server [Encrypted with Nounce+1]
// client ----------- Close[] ----------> server [Encrypted with Nounce+2]

// Bump protocol version if breaking change is introduced to the network protocol.
const PROTOCOL_VERSION: u32 = 1;

#[derive(Debug, Copy, Clone, FromPrimitive, ToPrimitive)]
enum NetFrameType {
    Open = 0,
    Close = 1,
    Message = 2,
}

#[derive(Debug)]
struct NetFrame {
    /// Payload protocol version used
    protocol_version: u32,
    /// Type of frame sent
    frame_type: NetFrameType,
    /// Size of payload
    payload_size: u64,
    /// Content of payload
    payload: Vec<u8>,
}

fn read_protocol_version(header: &[u8]) -> Result<u32, Error> {
    let protocol_version = u32::from_le_bytes(
        header[0..4]
            .try_into()
            .expect("Slice with incorrect length"),
    );
    if protocol_version != PROTOCOL_VERSION {
        error!(
            "Invalid protocol version message received. \
            Make sure client and server are using the same version. \
            Received: {}, Current {}",
            protocol_version, PROTOCOL_VERSION
        );
        Err(Error::from(ErrorKind::Unsupported))
    } else {
        Ok(protocol_version)
    }
}

fn read_frame_type(header: &[u8]) -> Result<NetFrameType, Error> {
    let frame_type_num = u32::from_le_bytes(
        header[4..8]
            .try_into()
            .expect("Slice with incorrect length"),
    );
    let frame_type: NetFrameType = match num_traits::FromPrimitive::from_u32(frame_type_num) {
        Some(ft) => ft,
        None => {
            error!(
                "Could not parse frame type. Frame type received: {}",
                frame_type_num
            );
            return Err(Error::from(std::io::ErrorKind::InvalidData));
        }
    };
    Ok(frame_type)
}

fn read_payload_size(header: &[u8]) -> Result<u64, Error> {
    Ok(u64::from_le_bytes(
        header[8..16]
            .try_into()
            .expect("slice with incorrect lenght"),
    ))
}

impl NetFrame {
    /// Export a netframe to vector stream
    fn to_net(&self) -> Vec<u8> {
        let mut vector = Vec::with_capacity(self.payload_size as usize);
        vector.extend_from_slice(&self.protocol_version.to_le_bytes());
        vector.extend_from_slice(
            &(num_traits::ToPrimitive::to_u32(&self.frame_type)
                .unwrap()
                .to_le_bytes()),
        );
        vector.extend_from_slice(&self.payload_size.to_le_bytes());
        vector.extend_from_slice(&self.payload);
        vector
    }

    /// Read NetFrame from a network stream
    fn from_net<T: Read>(reader: &mut T) -> Result<Self, Error> {
        let mut header_buffer: [u8; 4 + 4 + 8] = [0; 16];
        match reader.read_exact(&mut header_buffer) {
            Ok(_) => (),
            Err(e) => {
                error!("End of stream while reading header");
                return Err(e);
            }
        }

        let protocol_version = read_protocol_version(&header_buffer)?;
        let frame_type = read_frame_type(&header_buffer)?;
        let payload_size = read_payload_size(&header_buffer)?;

        trace!("NetFrame payload size: {}", payload_size);
        let mut payload = vec![0; payload_size as usize];
        reader.read_exact(&mut payload)?;

        Ok(NetFrame {
            protocol_version,
            frame_type,
            payload_size,
            payload,
        })
    }

    /// Close frame should be the last frame sent
    fn close_frame() -> NetFrame {
        Self {
            protocol_version: PROTOCOL_VERSION,
            frame_type: NetFrameType::Close,
            payload_size: 0,
            payload: vec![],
        }
    }
}

impl From<&[u8]> for NetFrame {
    fn from(message: &[u8]) -> Self {
        NetFrame {
            protocol_version: PROTOCOL_VERSION,
            frame_type: NetFrameType::Message,
            payload_size: message.len() as u64,
            payload: message.to_vec(),
        }
    }
}
