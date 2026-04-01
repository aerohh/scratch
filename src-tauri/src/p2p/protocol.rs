// Sync protocol for file sharing

use crate::p2p::types::SyncMessage;
use libp2p::{
    futures::prelude::*,
    request_response::{self, ProtocolSupport, ResponseChannel},
    swarm::NetworkBehaviour,
    StreamProtocol,
};
use std::io;
use async_trait::async_trait;

/// The sync protocol name
pub const SYNC_PROTOCOL: StreamProtocol = StreamProtocol::new("/scratch-sync/1.0.0");

/// Custom behaviour for sync protocol
#[derive(NetworkBehaviour)]
pub struct SyncBehaviour {
    request_response: request_response::Behaviour<SyncCodec>,
}

impl SyncBehaviour {
    /// Create a new sync behaviour
    pub fn new() -> Self {
        let cfg = request_response::Config::default().with_max_concurrent_streams(10);

        let request_response = request_response::Behaviour::new(
            [(SYNC_PROTOCOL, ProtocolSupport::Full)],
            cfg,
        );

        Self { request_response }
    }
}

/// Codec for encoding/decoding SyncMessage
#[derive(Default, Clone)]
pub struct SyncCodec;

#[async_trait]
impl request_response::Codec for SyncCodec {
    type Protocol = StreamProtocol;
    type Request = SyncMessage;
    type Response = SyncMessage;

    async fn read_request<T>(&mut self, _: &Self::Protocol, io: &mut T) -> io::Result<SyncMessage>
    where
        T: AsyncRead + Unpin + Send,
    {
        // Read message length prefix (4 bytes)
        let mut len_buf = [0u8; 4];
        io.read_exact(&mut len_buf).await?;
        let len = u32::from_be_bytes(len_buf) as usize;

        // Read message
        let mut buf = vec![0u8; len];
        io.read_exact(&mut buf).await?;

        // Deserialize
        let message: SyncMessage = serde_json::from_slice(&buf).map_err(|e| {
            io::Error::new(io::ErrorKind::InvalidData, format!("Failed to deserialize: {}", e))
        })?;

        Ok(message)
    }

    async fn read_response<T>(&mut self, protocol: &Self::Protocol, io: &mut T) -> io::Result<SyncMessage>
    where
        T: AsyncRead + Unpin + Send,
    {
        self.read_request(protocol, io).await
    }

    async fn write_request<T>(&mut self, _: &Self::Protocol, io: &mut T, message: SyncMessage) -> io::Result<()>
    where
        T: AsyncWrite + Unpin + Send,
    {
        // Serialize message
        let data = serde_json::to_vec(&message).map_err(|e| {
            io::Error::new(io::ErrorKind::InvalidData, format!("Failed to serialize: {}", e))
        })?;

        // Write length prefix
        let len = data.len() as u32;
        io.write_all(&len.to_be_bytes()).await?;

        // Write message
        io.write_all(&data).await?;
        io.flush().await?;

        Ok(())
    }

    async fn write_response<T>(&mut self, protocol: &Self::Protocol, io: &mut T, message: SyncMessage) -> io::Result<()>
    where
        T: AsyncWrite + Unpin + Send,
    {
        self.write_request(protocol, io, message).await
    }
}

/// Response channel for sending responses
pub type SyncResponseChannel = ResponseChannel<SyncMessage>;

/// Sync protocol events
pub enum SyncEvent {
    Request {
        peer: libp2p::PeerId,
        request: SyncMessage,
        channel: SyncResponseChannel,
    },
    Response {
        peer: libp2p::PeerId,
        response: SyncMessage,
    },
}

/// Helper to create error messages
pub fn error_message(message: String) -> SyncMessage {
    SyncMessage::Error { message }
}

/// Helper to create manifest request
pub fn manifest_request(share_id: String) -> SyncMessage {
    SyncMessage::RequestManifest { share_id }
}

/// Helper to create manifest response
pub fn manifest_response(share_id: String, files: Vec<crate::p2p::types::FileManifest>) -> SyncMessage {
    SyncMessage::Manifest { share_id, files }
}

/// Helper to create file request
pub fn file_request(share_id: String, path: String) -> SyncMessage {
    SyncMessage::RequestFile { share_id, path }
}

/// Helper to create file chunk response
pub fn file_chunk(share_id: String, path: String, offset: u64, data: Vec<u8>, is_final: bool) -> SyncMessage {
    SyncMessage::FileChunk {
        share_id,
        path,
        offset,
        data,
        is_final,
    }
}

/// Helper to create file changed notification
pub fn file_changed(share_id: String, path: String, is_deleted: bool) -> SyncMessage {
    SyncMessage::FileChanged {
        share_id,
        path,
        is_deleted,
    }
}

/// Helper to create sync complete message
pub fn sync_complete(has_conflicts: bool) -> SyncMessage {
    SyncMessage::SyncComplete { has_conflicts }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::p2p::types::FileManifest;

    #[test]
    fn test_message_helpers() {
        let msg = manifest_request("share-1".to_string());
        assert!(matches!(msg, SyncMessage::RequestManifest { .. }));

        let files = vec![FileManifest {
            path: "test.md".to_string(),
            hash: "abc123".to_string(),
            size: 100,
            modified: 12345,
            is_deleted: false,
        }];
        let msg = manifest_response("share-1".to_string(), files);
        assert!(matches!(msg, SyncMessage::Manifest { .. }));

        let msg = file_request("share-1".to_string(), "test.md".to_string());
        assert!(matches!(msg, SyncMessage::RequestFile { .. }));

        let msg = file_chunk("share-1".to_string(), "test.md".to_string(), 0, vec![1, 2, 3], true);
        assert!(matches!(msg, SyncMessage::FileChunk { .. }));

        let msg = file_changed("share-1".to_string(), "test.md".to_string(), false);
        assert!(matches!(msg, SyncMessage::FileChanged { .. }));

        let msg = sync_complete(false);
        assert!(matches!(msg, SyncMessage::SyncComplete { .. }));

        let msg = error_message("test error".to_string());
        assert!(matches!(msg, SyncMessage::Error { .. }));
    }
}
