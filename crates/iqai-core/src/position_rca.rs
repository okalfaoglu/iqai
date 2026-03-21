//! Kapalı pozisyon RCA (TFAI-Q01/Q02): canonical `close_reason`, açılış/kapanış meta alanları.
//!
//! TFAI-Q01 enterprise: VWAP, notional, R:R, süreler, ücret öncesi/sonrası PnL, bps, emir kimlikleri.

/// RCA: açılışta `positions` satırına yazılan TFAI-Q01 alanları.
#[derive(Debug, Clone)]
pub struct PositionOpenRca {
    pub position_uuid: String,
    pub strategy_id: String,
    pub exchange: String,
    pub trace_id: Option<String>,
    /// Unix epoch mikrosaniye (μs); sinyal zamanı ms ise `ms * 1000` ile üretilir.
    pub opened_at_us: i64,
    pub entry_slippage_bps: f64,
    /// Sinyal ateşlendiğinde referans fiyat (genelde `signal.entry`).
    pub signal_mid_price: f64,
    /// Gerçekleşen ortalama giriş (tek fill veya VWAP; kısmi doldurmalarda güncellenir).
    pub entry_price_avg: f64,
    /// Açılış anı USDT notional (|entry| × qty × contract multiplier yoksa qty×entry).
    pub position_notional_usd: f64,
    /// Konfigürasyondan etkin kaldıraç (risk boyutu için kullanılan üst sınır).
    pub leverage: u32,
    /// Sinyal üzerindeki R:R (açılış anı).
    pub rr_at_open: f64,
    /// Sinyal zaman damgasından (ms) ilk fill’e kadar geçen süre (ms).
    pub signal_to_entry_ms: i64,
    /// Opsiyonel: açılıştaki volatilite göstergesi (ör. ATR/price); yoksa NULL.
    pub volatility_at_open: Option<f64>,
    /// Opsiyonel: emir defteri spread (bps); borsa yoksa NULL.
    pub spread_at_open_bps: Option<f64>,
    /// Opsiyonel: perpetual funding (yoksa NULL).
    pub funding_rate_at_open: Option<f64>,
}

/// RCA: kapanışta güncellenen alanlar.
#[derive(Debug, Clone)]
pub struct ClosePositionRca {
    pub exit_slippage_bps: f64,
    pub mae_usd: f64,
    pub mfe_usd: f64,
    pub fees_total_usd: f64,
    pub trace_id: Option<String>,
    /// Ücret öncesi PnL (bu kapanış dilimi için).
    pub pnl_gross_usd: f64,
    /// Ücret sonrası net PnL (`positions.pnl` ile uyumlu olmalı).
    pub pnl_net_usd: f64,
    /// Normalize: `1e4 * pnl_net / max(notional_leg, ε)`.
    pub pnl_bps: f64,
    /// Çıkış VWAP / efektif ortalama fiyat.
    pub exit_price_avg: f64,
    /// Sinyal zamanından (ms) son fill’e kadar toplam süre (ms).
    pub lifecycle_duration_ms: i64,
    /// Kapanış borsa emri kimliği (canlı); simülasyonda sentetik.
    pub close_order_id: Option<String>,
    /// JSON: `["entry_order_id","close_order_id"]` — denetim izi.
    pub exit_orders_json: String,
}

/// Eski serbest metin veya kısaltılmış nedenleri TFAI dot-notation canonical koda çevirir (O-02).
/// Zaten `exit.` ile başlayan değerler aynen döner.
pub fn close_reason_to_canonical(reason: &str) -> String {
    let t = reason.trim();
    if t.starts_with("exit.") {
        return t.to_string();
    }
    match t {
        "Stop Loss" | "stop_loss" | "SL" => return "exit.sl.initial".to_string(),
        "Take Profit" | "take_profit" | "TP" => return "exit.tp.full".to_string(),
        "Trailing Stop" | "trailing_stop" => return "exit.sl.trailing".to_string(),
        "Position remaining_pct<=0" => return "exit.system.position_sync".to_string(),
        _ => {}
    }
    if t.contains("TP1") || t.contains("TP2") || t.contains("kısmi") || t.to_lowercase().contains("partial") {
        return "exit.tp.partial".to_string();
    }
    if t.contains("Zıt") {
        return "exit.strategy.signal_reversal".to_string();
    }
    if t.contains("Breakeven") {
        return "exit.sl.initial".to_string();
    }
    "exit.manual.operator".to_string()
}

#[cfg(test)]
mod close_reason_tests {
    use super::close_reason_to_canonical;

    #[test]
    fn canonical_pass_through_exit_dot() {
        assert_eq!(
            close_reason_to_canonical("exit.sl.initial"),
            "exit.sl.initial"
        );
        assert_eq!(
            close_reason_to_canonical("  exit.tp.partial  "),
            "exit.tp.partial"
        );
    }

    #[test]
    fn legacy_stop_take_trailing() {
        assert_eq!(close_reason_to_canonical("Stop Loss"), "exit.sl.initial");
        assert_eq!(close_reason_to_canonical("Take Profit"), "exit.tp.full");
        assert_eq!(close_reason_to_canonical("Trailing Stop"), "exit.sl.trailing");
    }

    #[test]
    fn legacy_tp_partial_and_zit() {
        assert_eq!(close_reason_to_canonical("TP1 (1R)"), "exit.tp.partial");
        assert_eq!(
            close_reason_to_canonical("Zıt yön — yeni sinyal"),
            "exit.strategy.signal_reversal"
        );
    }

    #[test]
    fn unknown_maps_to_manual_operator() {
        assert_eq!(close_reason_to_canonical("random note"), "exit.manual.operator");
    }
}
