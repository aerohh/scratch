// Peer discovery using mDNS

use anyhow::Result;
use libp2p::{
    mdns,
    swarm::{NetworkBehaviour, ToSwarm},
    PeerId,
};
use std::time::Duration;

/// Custom behaviour combining mDNS for local discovery
#[derive(NetworkBehaviour)]
pub struct DiscoveryBehaviour {
    mdns: mdns::tokio::Behaviour,
}

impl DiscoveryBehaviour {
    /// Create a new discovery behaviour with mDNS
    pub fn new(local_peer_id: PeerId) -> Result<Self> {
        let mdns = mdns::tokio::Behaviour::new(mdns::Config::default(), local_peer_id)
            .map_err(|e| anyhow::anyhow!("Failed to create mDNS behaviour: {}", e))?;

        Ok(Self { mdns })
    }

    /// Handle discovery events
    pub fn on_event(&mut self, event: DiscoveryEvent) {
        match event {
            DiscoveryEvent::Discovered { peer_id, addresses } => {
                log::info!("Discovered peer: {} at {:?}", peer_id, addresses);
            }
            DiscoveryEvent::Expired { peer_id } => {
                log::info!("Peer expired: {}", peer_id);
            }
        }
    }
}

/// Discovery events
pub enum DiscoveryEvent {
    Discovered {
        peer_id: PeerId,
        addresses: Vec<libp2p::Multiaddr>,
    },
    Expired {
        peer_id: PeerId,
    },
}

/// Discovery manager for handling peer discovery
pub struct DiscoveryManager {
    peer_id: PeerId,
    discovered_peers: std::collections::HashMap<PeerId, Vec<libp2p::Multiaddr>>,
}

impl DiscoveryManager {
    pub fn new(peer_id: PeerId) -> Self {
        Self {
            peer_id,
            discovered_peers: std::collections::HashMap::new(),
        }
    }

    /// Get all discovered peers
    pub fn get_discovered_peers(&self) -> &std::collections::HashMap<PeerId, Vec<libp2p::Multiaddr>> {
        &self.discovered_peers
    }

    /// Add a discovered peer
    pub fn add_discovered_peer(&mut self, peer_id: PeerId, addresses: Vec<libp2p::Multiaddr>) {
        self.discovered_peers.insert(peer_id, addresses);
        log::info!("Added discovered peer: {}", peer_id);
    }

    /// Remove an expired peer
    pub fn remove_expired_peer(&mut self, peer_id: &PeerId) {
        self.discovered_peers.remove(peer_id);
        log::info!("Removed expired peer: {}", peer_id);
    }

    /// Check if a peer is known
    pub fn knows_peer(&self, peer_id: &PeerId) -> bool {
        self.discovered_peers.contains_key(peer_id)
    }

    /// Get addresses for a peer
    pub fn get_peer_addresses(&self, peer_id: &PeerId) -> Option<&[libp2p::Multiaddr]> {
        self.discovered_peers.get(peer_id).map(|v| v.as_slice())
    }

    /// Get the number of discovered peers
    pub fn peer_count(&self) -> usize {
        self.discovered_peers.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_discovery_manager() {
        let peer_id = PeerId::random();
        let mut manager = DiscoveryManager::new(peer_id);

        let test_peer = PeerId::random();
        let addr = "/ip4/127.0.0.1/tcp/8000".parse().unwrap();

        manager.add_discovered_peer(test_peer, vec![addr.clone()]);

        assert!(manager.knows_peer(&test_peer));
        assert_eq!(manager.peer_count(), 1);
        assert_eq!(manager.get_peer_addresses(&test_peer), Some(vec![addr.as_ref()].as_slice()));

        manager.remove_expired_peer(&test_peer);

        assert!(!manager.knows_peer(&test_peer));
        assert_eq!(manager.peer_count(), 0);
    }
}
