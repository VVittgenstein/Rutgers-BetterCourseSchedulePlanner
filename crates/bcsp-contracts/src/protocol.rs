use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ProtocolVersion {
    V1,
}

pub const API_PROTOCOL_VERSION: ProtocolVersion = ProtocolVersion::V1;
pub const WS_PROTOCOL_VERSION: ProtocolVersion = API_PROTOCOL_VERSION;

impl ProtocolVersion {
    pub const fn as_u16(self) -> u16 {
        match self {
            Self::V1 => 1,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
#[error("unsupported protocol version")]
pub struct ProtocolVersionError {
    observed: u16,
}

impl ProtocolVersionError {
    pub const fn observed(self) -> u16 {
        self.observed
    }
}

impl TryFrom<u16> for ProtocolVersion {
    type Error = ProtocolVersionError;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::V1),
            observed => Err(ProtocolVersionError { observed }),
        }
    }
}

impl From<ProtocolVersion> for u16 {
    fn from(value: ProtocolVersion) -> Self {
        value.as_u16()
    }
}

impl Serialize for ProtocolVersion {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u16(self.as_u16())
    }
}

impl<'de> Deserialize<'de> for ProtocolVersion {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = u16::deserialize(deserializer)?;
        Self::try_from(value).map_err(D::Error::custom)
    }
}
