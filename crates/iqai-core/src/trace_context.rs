//! W3C Trace Context (`traceparent`) — dağıtık iz ve OTel ile uyum için.
//!
//! IQAI içi `trace_id` UUID metni ile uyum: 32 hex (tireler atılır) → W3C `trace-id` alanı.

/// `trace_id` UUID metninden (veya 32 hex) W3C `traceparent` değeri üretir.
///
/// Biçim: `00-{trace-id}-{parent-id}-{flags}`  
/// - `trace-id`: 32 hex (128 bit)  
/// - `parent-id`: kök span için sabit `0000000000000001` (16 hex)  
/// - `flags`: `01` (sampled)
///
/// Geçersiz uzunlukta `None` döner.
pub fn traceparent_from_uuid(trace_id: &str) -> Option<String> {
    let hex: String = trace_id
        .chars()
        .filter(|c| c.is_ascii_hexdigit())
        .collect();
    if hex.len() != 32 {
        return None;
    }
    // Kök span: parent-id sabit (uygulama içi tek segment; alt span’lar OTel ile gelecek).
    const PARENT_ROOT: &str = "0000000000000001";
    const FLAGS: &str = "01";
    Some(format!("00-{hex}-{PARENT_ROOT}-{FLAGS}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn traceparent_from_standard_uuid() {
        let tp = traceparent_from_uuid("550e8400-e29b-41d4-a716-446655440000").expect("tp");
        assert_eq!(
            tp,
            "00-550e8400e29b41d4a716446655440000-0000000000000001-01"
        );
    }

    #[test]
    fn traceparent_invalid_length() {
        assert!(traceparent_from_uuid("short").is_none());
    }
}
