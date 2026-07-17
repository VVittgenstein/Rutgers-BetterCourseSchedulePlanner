use serde::{Deserialize, Serialize};

use crate::{API_PROTOCOL_VERSION, ProtocolVersion, TraceId, WS_PROTOCOL_VERSION};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HttpRequestEnvelope<T> {
    protocol_version: ProtocolVersion,
    payload: T,
}

impl<T> HttpRequestEnvelope<T> {
    pub fn new(payload: T) -> Self {
        Self {
            protocol_version: API_PROTOCOL_VERSION,
            payload,
        }
    }

    pub const fn protocol_version(&self) -> ProtocolVersion {
        self.protocol_version
    }

    pub const fn payload(&self) -> &T {
        &self.payload
    }

    pub fn into_payload(self) -> T {
        self.payload
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HttpSuccessEnvelope<T> {
    protocol_version: ProtocolVersion,
    data: T,
}

impl<T> HttpSuccessEnvelope<T> {
    pub fn new(data: T) -> Self {
        Self {
            protocol_version: API_PROTOCOL_VERSION,
            data,
        }
    }

    pub const fn protocol_version(&self) -> ProtocolVersion {
        self.protocol_version
    }

    pub const fn data(&self) -> &T {
        &self.data
    }

    pub fn into_data(self) -> T {
        self.data
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WsClientEnvelope<T> {
    protocol_version: ProtocolVersion,
    message_id: TraceId,
    payload: T,
}

impl<T> WsClientEnvelope<T> {
    pub fn new(message_id: TraceId, payload: T) -> Self {
        Self {
            protocol_version: WS_PROTOCOL_VERSION,
            message_id,
            payload,
        }
    }

    pub const fn protocol_version(&self) -> ProtocolVersion {
        self.protocol_version
    }

    pub const fn message_id(&self) -> TraceId {
        self.message_id
    }

    pub const fn payload(&self) -> &T {
        &self.payload
    }

    pub fn into_payload(self) -> T {
        self.payload
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WsServerEnvelope<T> {
    protocol_version: ProtocolVersion,
    message_id: TraceId,
    payload: T,
}

impl<T> WsServerEnvelope<T> {
    pub fn new(message_id: TraceId, payload: T) -> Self {
        Self {
            protocol_version: WS_PROTOCOL_VERSION,
            message_id,
            payload,
        }
    }

    pub const fn protocol_version(&self) -> ProtocolVersion {
        self.protocol_version
    }

    pub const fn message_id(&self) -> TraceId {
        self.message_id
    }

    pub const fn payload(&self) -> &T {
        &self.payload
    }

    pub fn into_payload(self) -> T {
        self.payload
    }
}
