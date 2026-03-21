//! SHA-256 yardımcıları (AI prompt / bağlam parmak izi — TFAI-O08).

use sha2::{Digest, Sha256};

/// Verinin SHA-256 özetini küçük harf hex string olarak döndürür.
pub fn sha256_hex(data: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(data);
    hex::encode(h.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_known_empty() {
        let s = sha256_hex(b"");
        assert_eq!(
            s,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }
}
