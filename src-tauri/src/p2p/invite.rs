// Invite code generation and parsing

use crate::p2p::types::{InvitePayload, INVITE_PREFIX, INVITE_VERSION};
use anyhow::{Context, Result};
use base64::{engine::general_purpose::STANDARD, Engine};
use rand::Rng;
use std::time::{SystemTime, UNIX_EPOCH};

/// Generate a random encryption key (base64 encoded)
pub fn generate_encryption_key() -> String {
    let key: [u8; 32] = rand::thread_rng().gen();
    STANDARD.encode(key)
}

/// Generate a unique share ID
pub fn generate_share_id() -> String {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let random: u32 = rand::thread_rng().gen();
    format!("share-{}-{:08x}", timestamp, random)
}

/// Generate a unique folder ID
pub fn generate_folder_id() -> String {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let random: u32 = rand::thread_rng().gen();
    format!("folder-{}-{:08x}", timestamp, random)
}

/// Create an invite payload
pub fn create_invite_payload(
    sender_peer_id: String,
    folder_name: String,
    permissions: crate::p2p::types::SharePermission,
    source_path: Option<String>,
) -> Result<InvitePayload> {
    let encryption_key = generate_encryption_key();
    let folder_id = generate_folder_id();
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("Failed to get timestamp")?
        .as_secs() as i64;

    Ok(InvitePayload {
        version: INVITE_VERSION,
        sender_peer_id,
        folder_id,
        folder_name,
        permissions,
        encryption_key,
        timestamp,
        relay_hint: None, // Phase 3
        source_path,
    })
}

/// Encode invite payload to invite code
///
/// Format: scratch-share-<base64_payload>
/// Note: In production, the payload should be encrypted before encoding
pub fn encode_invite_code(payload: &InvitePayload) -> Result<String> {
    let json = serde_json::to_string(payload).context("Failed to serialize payload")?;

    // TODO: Phase 2 - Encrypt with Noise protocol before encoding
    // For now, we'll use base64 encoding directly
    let encoded = STANDARD.encode(json.as_bytes());

    Ok(format!("{}{}", INVITE_PREFIX, encoded))
}

/// Decode invite code to payload
///
/// Note: In production, the payload should be decrypted after decoding
pub fn decode_invite_code(invite_code: &str) -> Result<InvitePayload> {
    if !invite_code.starts_with(INVITE_PREFIX) {
        anyhow::bail!("Invalid invite code format");
    }

    let encoded = &invite_code[INVITE_PREFIX.len()..];

    // TODO: Phase 2 - Decrypt after decoding
    let decoded_bytes = STANDARD
        .decode(encoded)
        .context("Failed to decode invite code (invalid base64)")?;

    let json_string = std::str::from_utf8(&decoded_bytes)
        .context("Failed to parse invite code (invalid UTF-8)")?;

    let payload: InvitePayload = serde_json::from_str(json_string)
        .context("Failed to parse invite payload (invalid JSON)")?;

    // Verify version
    if payload.version != INVITE_VERSION {
        anyhow::bail!(
            "Unsupported invite version: {} (expected {})",
            payload.version,
            INVITE_VERSION
        );
    }

    Ok(payload)
}

/// Validate invite code format
pub fn validate_invite_code(invite_code: &str) -> Result<()> {
    decode_invite_code(invite_code)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::p2p::types::SharePermission;

    #[test]
    fn test_invite_encode_decode() {
        let payload = create_invite_payload(
            "12D3KooWTestPeerId".to_string(),
            "TestFolder".to_string(),
            SharePermission::ReadWrite,
            None,
        )
        .unwrap();

        let code = encode_invite_code(&payload).unwrap();
        assert!(code.starts_with(INVITE_PREFIX));

        let decoded = decode_invite_code(&code).unwrap();
        assert_eq!(decoded.sender_peer_id, "12D3KooWTestPeerId");
        assert_eq!(decoded.folder_name, "TestFolder");
        assert_eq!(decoded.permissions, SharePermission::ReadWrite);
    }

    #[test]
    fn test_invalid_invite_code() {
        let result = decode_invite_code("invalid-code");
        assert!(result.is_err());
    }

    #[test]
    fn test_generate_share_id() {
        let id1 = generate_share_id();
        let id2 = generate_share_id();
        assert_ne!(id1, id2);
        assert!(id1.starts_with("share-"));
    }
}
