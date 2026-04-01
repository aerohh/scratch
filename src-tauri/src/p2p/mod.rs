// P2P folder sharing module

pub mod invite;
pub mod protocol;
pub mod discovery;
pub mod network;
pub mod sync;
pub mod types;

// Re-export commonly used types
pub use types::{
    CreateShareResult, P2PState, P2PStatus, SharePermission, SharedFolder, SyncStatus,
};

pub use invite::{
    create_invite_payload, decode_invite_code, encode_invite_code, generate_share_id,
};

pub use network::NetworkManager;
