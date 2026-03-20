//! İleride Q-Analiz / dip-tepe pipeline’a eklenebilecek ek piyasa verileri.
//!
//! Şu an hesaplamada **kullanılmıyor**; sadece tipler ve (Binance tarafında) veri çekme
//! hazır. İleride: destek/direnç, funding baskısı, likidite bölgeleri, on-chain bağlam.

use serde::{Deserialize, Serialize};

/// Order book özeti – alış/satış yoğunluğu (belirli seviyeye kadar toplam notional).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OrderBookSnapshot {
    /// Toplam alış (bid) notional – örn. ilk 20 seviye
    pub bid_notional: f64,
    /// Toplam satış (ask) notional
    pub ask_notional: f64,
    /// (bid - ask) / (bid + ask) → -1..1; pozitif = alış ağırlıklı
    pub imbalance: f64,
}

/// Funding rate (futures) – son periyot.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FundingRate {
    /// Oran (örn. 0.0001 = %0.01)
    pub rate: f64,
    /// Bir sonraki funding zamanı (ms)
    pub next_funding_time: Option<i64>,
}

/// Açık pozisyon (open interest) – futures.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OpenInterest {
    pub value: f64,
    /// Önceki ölçüme göre değişim (isteğe bağlı)
    pub change_pct: Option<f64>,
}

/// Tahmini likidasyon bölgesi – basit band (gerçek likidite verisi yoksa tahmin).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LiquidationZone {
    /// Long likidasyonları (fiyat altına düşünce stop’lar tetiklenir) – band alt sınırı
    pub long_liq_low: Option<f64>,
    pub long_liq_high: Option<f64>,
    /// Short likidasyonları – band üst sınırı
    pub short_liq_low: Option<f64>,
    pub short_liq_high: Option<f64>,
}

/// On-chain özet (placeholder – gerçek veri harici API’den gelir).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OnChainSummary {
    /// Kısa metin (örn. "Borsa girişi artış", "Whale hareketi")
    pub note: Option<String>,
    /// İsteğe bağlı sayısal gösterge (0–1 veya skor)
    pub score: Option<f64>,
}

/// Tüm ek piyasa bağlamı – Q-Analiz’e opsiyonel girdi.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MarketContext {
    pub order_book: Option<OrderBookSnapshot>,
    pub funding_rate: Option<FundingRate>,
    pub open_interest: Option<OpenInterest>,
    pub liquidation_zones: Option<LiquidationZone>,
    pub on_chain: Option<OnChainSummary>,
}
