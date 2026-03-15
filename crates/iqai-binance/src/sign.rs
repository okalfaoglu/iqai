//! Binance HMAC-SHA256 signing for authenticated endpoints.

use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

/// HMAC-SHA256 signature of `query_string` using the given `secret`.
pub fn sign(query_string: &str, secret: &str) -> String {
    let mut mac =
        HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC can take key of any size");
    mac.update(query_string.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

/// Current server-compatible timestamp (ms since epoch).
pub fn timestamp_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sign_deterministic() {
        let sig = sign("symbol=BTCUSDT&side=BUY&type=MARKET&quantity=0.01&timestamp=1234567890", "secret123");
        assert_eq!(sig.len(), 64);
    }
}
