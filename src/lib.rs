use log::{error, trace};
use num_derive::{FromPrimitive, ToPrimitive};
use rand::prelude::*;
use std::io::{Error, ErrorKind, Read};

pub mod client;
pub mod server;

/// Protocol (wanted):
/// client ----------- Open[] -----------> server
/// client <-------- Open[Nounce] -------- server
/// client ------ Message[[u8]] ------> server [Encrypted with Nounce]
/// client ------ Message[[u8]] ------> server [Encrypted with Nounce+1]
/// client ----------- Close[] ----------> server [Encrypted with Nounce+2]

// Client states:
// Start -> Opening -> Opened -> Closed

// Bump protocol version if breaking change is introduced to the network protocol.
const PROTOCOL_VERSION: u32 = 1;
pub const NOUNCE_SIZE: usize = 12;
pub const KEY_SIZE: usize = 32;

// deciphered close payload
pub const CLOSE_PAYLOAD: [u8; 1] = [b'c'];

#[derive(Debug, Clone, Copy)]
pub struct Nonce {
    value: [u8; NOUNCE_SIZE],
}

pub type Cipher = chacha20poly1305::ChaCha20Poly1305;

impl Nonce {
    /// Consume the nounce and return a new one that has not been used yet
    pub fn consume(self) -> Self {
        let mut value = self.value;
        for i in (0..value.len()).rev() {
            if let Some(v) = value[i].checked_add(1) {
                value[i] = v;
                break;
            } else {
                value[i] = 0;
            }
        }
        Self { value }
    }

    /// Get Nonce reference digestable by current cipher.
    pub fn cipher_nonce(&self) -> &chacha20poly1305::Nonce {
        chacha20poly1305::Nonce::from_slice(&self.value)
    }
}

impl Default for Nonce {
    /// Create a new nonce with a random value
    fn default() -> Self {
        Self { value: random() }
    }
}
impl From<[u8; NOUNCE_SIZE]> for Nonce {
    fn from(value: [u8; NOUNCE_SIZE]) -> Self {
        Self { value }
    }
}

impl TryFrom<Vec<u8>> for Nonce {
    type Error = Vec<u8>;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        Ok(Self {
            value: value.try_into()?,
        })
    }
}

#[derive(Debug)]
pub enum ConnectionState {
    New,
    Opened(Nonce),
    Closed,
}

#[derive(Debug, Copy, Clone, FromPrimitive, ToPrimitive)]
enum NetFrameType {
    /// Open new connection
    Open = 0,
    /// Close connection
    Close = 1,
    /// Send a message
    CopyMessage = 2,
    /// Send a non-copy message
    ExecMessage = 3,
}

type ProtocolVersionType = u32;
type FrameSizeType = u64;
type NetFrameTypeType = u32;

const PROTOCOL_VERSION_SIZE: usize = std::mem::size_of::<ProtocolVersionType>();
const FRAME_SIZE_SIZE: usize = std::mem::size_of::<FrameSizeType>();
const FRAME_TYPE_SIZE: usize = std::mem::size_of::<NetFrameTypeType>();

/// Netframe representation on network:
/// | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 |
/// | ----------------------------- |
/// | prot version  | frame_size... |
/// | ...frame_size | frame_type    |
/// | ----------------------------- |
/// |             payload           |
/// |                               |
/// | ----------------------------- |
#[derive(Debug)]
struct NetFrame {
    /// Payload protocol version used
    protocol_version: ProtocolVersionType,
    /// Size of header and payload
    frame_size: FrameSizeType,
    /// Type of frame sent
    frame_type: NetFrameType,
    /// Content of payload
    payload: Vec<u8>,
}

fn read_protocol_version(header: &[u8]) -> Result<u32, Error> {
    const OFFSET: usize = 0;
    const WIDTH: usize = PROTOCOL_VERSION_SIZE;
    let protocol_version = ProtocolVersionType::from_le_bytes(
        header[OFFSET..OFFSET + WIDTH]
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

fn read_frame_size(header: &[u8]) -> Result<u64, Error> {
    const OFFSET: usize = PROTOCOL_VERSION_SIZE;
    const WIDTH: usize = FRAME_SIZE_SIZE;
    Ok(FrameSizeType::from_le_bytes(
        header[OFFSET..OFFSET + WIDTH]
            .try_into()
            .expect("slice with incorrect length"),
    ))
}

fn read_frame_type(payload: &[u8]) -> Result<NetFrameType, Error> {
    const OFFSET: usize = 0;
    const WIDTH: usize = FRAME_TYPE_SIZE;
    let frame_type_num = u32::from_le_bytes(
        payload[OFFSET..OFFSET + WIDTH]
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

impl NetFrame {
    fn new(frame_type: NetFrameType, payload: Vec<u8>) -> Self {
        Self {
            protocol_version: PROTOCOL_VERSION,
            frame_size: NetFrame::compute_frame_size(&payload),
            frame_type,
            payload,
        }
    }

    /// Export a netframe to vector stream
    fn to_net(&self) -> Vec<u8> {
        let mut vector = Vec::with_capacity(self.frame_size as usize);
        let frame_type = num_traits::ToPrimitive::to_u32(&self.frame_type)
            .unwrap()
            .to_le_bytes();
        vector.extend_from_slice(&self.protocol_version.to_le_bytes());
        vector.extend_from_slice(&self.frame_size.to_le_bytes());
        vector.extend_from_slice(&frame_type);
        vector.extend_from_slice(&self.payload);
        vector
    }

    /// Read NetFrame from a network stream
    /// Note: from_net does two read operation per frame (one for header, one for the
    /// payload), this might be inefficient on direct fd since it will trigger
    ///  syscalls on unbuffered readers.
    fn from_net<T: Read>(reader: &mut T) -> Result<Self, Error> {
        const HEADER_WIDTH: usize = PROTOCOL_VERSION_SIZE + FRAME_SIZE_SIZE;
        let mut header_buffer: [u8; HEADER_WIDTH] = [0; HEADER_WIDTH];
        match reader.read_exact(&mut header_buffer) {
            Ok(_) => (),
            Err(e) => {
                error!("End of stream while reading header");
                return Err(e);
            }
        }

        let protocol_version = read_protocol_version(&header_buffer)?;
        let frame_size = read_frame_size(&header_buffer)?;
        let payload_buffer_size = frame_size as usize - HEADER_WIDTH;

        trace!("NetFrame payload size: {}", payload_buffer_size);
        let mut payload_buffer = vec![0; payload_buffer_size];
        reader.read_exact(&mut payload_buffer)?;

        let frame_type = read_frame_type(&payload_buffer)?;
        let payload = payload_buffer.into_iter().skip(FRAME_TYPE_SIZE).collect();
        Ok(NetFrame {
            protocol_version,
            frame_size,
            frame_type,
            payload,
        })
    }

    /// Close frame should be the last frame sent
    fn close_frame(payload: Vec<u8>) -> NetFrame {
        Self {
            protocol_version: PROTOCOL_VERSION,
            frame_size: NetFrame::compute_frame_size(&payload),
            frame_type: NetFrameType::Close,
            payload,
        }
    }

    fn open_frame() -> NetFrame {
        let payload = Vec::with_capacity(0);
        Self {
            protocol_version: PROTOCOL_VERSION,
            frame_size: NetFrame::compute_frame_size(&payload),
            frame_type: NetFrameType::Open,
            payload,
        }
    }
    fn nounce_frame(nounce: &Nonce) -> NetFrame {
        let payload = nounce.value.to_vec();
        Self {
            protocol_version: PROTOCOL_VERSION,
            frame_size: NetFrame::compute_frame_size(&payload),
            frame_type: NetFrameType::Open,
            payload,
        }
    }

    fn compute_frame_size(payload: &[u8]) -> FrameSizeType {
        (PROTOCOL_VERSION_SIZE + FRAME_TYPE_SIZE + FRAME_SIZE_SIZE + payload.len())
            .try_into()
            .unwrap()
    }
}
