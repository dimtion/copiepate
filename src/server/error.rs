use thiserror::Error;

#[derive(Error, Debug)]
pub enum ServerError {
    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error("Invalid state")]
    InvalidState,

    #[error("Decryption error: {0}")]
    Decryption(chacha20poly1305::aead::Error),
}
