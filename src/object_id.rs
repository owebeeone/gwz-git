use std::fmt;

use crate::error::Error;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub enum ObjectFormat {
    Sha1,
    Sha256,
}

impl ObjectFormat {
    pub(crate) const fn byte_len(self) -> usize {
        match self {
            Self::Sha1 => 20,
            Self::Sha256 => 32,
        }
    }

    pub(crate) const fn hex_len(self) -> usize {
        self.byte_len() * 2
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ObjectId {
    format: ObjectFormat,
    bytes: [u8; 32],
}

impl ObjectId {
    pub fn parse_hex(format: ObjectFormat, text: &str) -> Result<Self, Error> {
        if text.len() != format.hex_len() {
            return Err(Error::invalid_id(format!(
                "object id must contain exactly {} hexadecimal digits",
                format.hex_len()
            )));
        }
        let mut bytes = [0u8; 32];
        for (index, pair) in text.as_bytes().chunks_exact(2).enumerate() {
            let high = hex_value(pair[0])
                .ok_or_else(|| Error::invalid_id("object id contains a non-hexadecimal byte"))?;
            let low = hex_value(pair[1])
                .ok_or_else(|| Error::invalid_id("object id contains a non-hexadecimal byte"))?;
            bytes[index] = (high << 4) | low;
        }
        Ok(Self { format, bytes })
    }

    pub fn format(&self) -> ObjectFormat {
        self.format
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes[..self.format.byte_len()]
    }

    pub(crate) fn from_git2(id: git2::Oid) -> Self {
        let format = match id.object_format() {
            git2::ObjectFormat::Sha1 => ObjectFormat::Sha1,
            git2::ObjectFormat::Sha256 => ObjectFormat::Sha256,
        };
        let mut bytes = [0u8; 32];
        bytes[..format.byte_len()].copy_from_slice(id.as_bytes());
        Self { format, bytes }
    }

    pub(crate) fn to_git2(self) -> git2::Oid {
        git2::Oid::from_bytes(self.as_bytes()).expect("validated object id")
    }
}

impl fmt::Display for ObjectId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.as_bytes() {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}
