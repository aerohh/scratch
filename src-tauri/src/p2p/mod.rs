// P2P folder sharing module

pub mod invite;
pub mod protocol;
pub mod discovery;
pub mod network;
pub mod sync;
pub mod types;
pub mod activity;

// Re-export commonly used types
pub use types::{
    ActivityEntry, ConnectionInfo, ConnectionType, CreateShareResult,
    P2PState, P2PStatus, PeerInfo, ShareMember, SharePermission, SharedFolder, SyncStatus,
};

pub use invite::{
    create_invite_payload, decode_invite_code, encode_invite_code,
};

pub use network::NetworkManager;

pub use activity::ActivityTracker;
