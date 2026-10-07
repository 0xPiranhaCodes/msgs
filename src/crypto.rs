use anyhow::{bail, Result};
use argon2::Argon2;
use hkdf::Hkdf;
use sha2::Sha256;
use zeroize::Zeroizing;

#[derive(Clone)]
pub struct RoomKeys {
    pub discovery: Zeroizing<[u8; 32]>,
    pub noise_psk: Zeroizing<[u8; 32]>,
}

pub fn derive_room_keys(passphrase: Zeroizing<Vec<u8>>) -> Result<RoomKeys> {
    if passphrase.is_empty() {
        bail!("passphrase must not be empty");
    }
    let mut root = Zeroizing::new([0_u8; 32]);
    Argon2::default()
        .hash_password_into(&passphrase, b"msgs/v2 room key", &mut *root)
        .map_err(|error| anyhow::anyhow!("derive room key: {error}"))?;
    let hkdf = Hkdf::<Sha256>::new(None, &*root);
    let mut discovery = Zeroizing::new([0_u8; 32]);
    let mut noise_psk = Zeroizing::new([0_u8; 32]);
    hkdf.expand(b"msgs/v2/discovery", &mut *discovery)
        .map_err(|_| anyhow::anyhow!("expand discovery key"))?;
    hkdf.expand(b"msgs/v2/noise-psk", &mut *noise_psk)
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
        let first = derive_room_keys(Zeroizing::new(b"strong shared passphrase".to_vec())).unwrap();
        let second =
            derive_room_keys(Zeroizing::new(b"strong shared passphrase".to_vec())).unwrap();
        assert_eq!(&*first.discovery, &*second.discovery);
        assert_eq!(&*first.noise_psk, &*second.noise_psk);
        assert_ne!(&*first.discovery, &*first.noise_psk);
    }

    #[test]
    fn rejects_empty_passphrase() {
        assert!(derive_room_keys(Zeroizing::new(Vec::new())).is_err());
    }
}
