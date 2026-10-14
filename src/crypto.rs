use anyhow::{bail, Result};
use hkdf::Hkdf;
use sha2::Sha256;
use zeroize::Zeroizing;

#[derive(Clone)]
pub struct RoomKeys {
    pub discovery: Zeroizing<[u8; 32]>,
    pub noise_psk: Zeroizing<[u8; 32]>,
}

/// Derives a room's discovery and Noise PSK keys from its room domain (see `CONTEXT.md`). The
/// room domain is public, not a secret, so this skips any slow/memory-hard hashing — there is no
/// brute-force resistance to buy.
pub fn derive_room_keys(room_domain: &str) -> Result<RoomKeys> {
    if room_domain.is_empty() {
        bail!("room domain must not be empty");
    }
    let hkdf = Hkdf::<Sha256>::new(None, room_domain.as_bytes());
    let mut discovery = Zeroizing::new([0_u8; 32]);
    let mut noise_psk = Zeroizing::new([0_u8; 32]);
    hkdf.expand(b"msgs/v3/discovery", &mut *discovery)
        .map_err(|_| anyhow::anyhow!("expand discovery key"))?;
    hkdf.expand(b"msgs/v3/noise-psk", &mut *noise_psk)
        .map_err(|_| anyhow::anyhow!("expand Noise PSK"))?;
    Ok(RoomKeys {
        discovery,
        noise_psk,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derives_deterministic_separated_keys() {
        let first = derive_room_keys("hackerearth.com").unwrap();
        let second = derive_room_keys("hackerearth.com").unwrap();
        assert_eq!(&*first.discovery, &*second.discovery);
        assert_eq!(&*first.noise_psk, &*second.noise_psk);
        assert_ne!(&*first.discovery, &*first.noise_psk);
    }

    #[test]
    fn different_domains_derive_different_rooms() {
        let first = derive_room_keys("hackerearth.com").unwrap();
        let second = derive_room_keys("example.com").unwrap();
        assert_ne!(&*first.discovery, &*second.discovery);
    }

    #[test]
    fn rejects_empty_domain() {
        assert!(derive_room_keys("").is_err());
    }
}
