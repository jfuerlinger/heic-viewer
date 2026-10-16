use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeError {
    /// Input is empty or larger than [`crate::Limits::max_input_bytes`].
    InputSize(usize),
    /// Image dimensions exceed [`crate::Limits::max_pixels`] or are zero.
    ImageTooLarge { width: u32, height: u32 },
    /// Error reported by libheif (corrupt file, unsupported codec, ...).
    Heif { code: i32, subcode: i32, message: String },
    /// Decoded data has a layout we did not ask for.
    UnexpectedLayout(&'static str),
    /// Decoding did not finish within [`crate::Limits::timeout`].
    Timeout,
    /// A panic was caught inside the decoder.
    Internal(String),
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DecodeError::InputSize(n) => write!(f, "input size {n} bytes is empty or exceeds the limit"),
            DecodeError::ImageTooLarge { width, height } => {
                write!(f, "image size {width}x{height} is empty or exceeds the limit")
            }
            DecodeError::Heif { code, subcode, message } => {
                write!(f, "libheif error {code}/{subcode}: {message}")
            }
            DecodeError::UnexpectedLayout(what) => write!(f, "unexpected decoded image layout: {what}"),
            DecodeError::Timeout => write!(f, "decoding timed out"),
            DecodeError::Internal(msg) => write!(f, "internal decoder error: {msg}"),
        }
    }
}

impl std::error::Error for DecodeError {}
