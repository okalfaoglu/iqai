//! Core data types for OHLCV and market structure

use serde::{Deserialize, Serialize};

/// Single candlestick (OHLCV)
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Candle {
    pub time: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
}

impl Candle {
    pub fn hlc3(&self) -> f64 {
        (self.high + self.low + self.close) / 3.0
    }

    pub fn typical_price(&self) -> f64 {
        self.hlc3()
    }

    pub fn is_bullish(&self) -> bool {
        self.close > self.open
    }

    pub fn is_bearish(&self) -> bool {
        self.close < self.open
    }
}

/// Timeframe in minutes (TradingView compatible)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Timeframe {
    M1,   // 1
    M5,   // 5
    M15,  // 15
    M30,  // 30
    H1,   // 60
    H4,   // 240
    D1,   // 1440
}

impl Timeframe {
    pub fn minutes(&self) -> u32 {
        match self {
            Timeframe::M1 => 1,
            Timeframe::M5 => 5,
            Timeframe::M15 => 15,
            Timeframe::M30 => 30,
            Timeframe::H1 => 60,
            Timeframe::H4 => 240,
            Timeframe::D1 => 1440,
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_uppercase().as_str() {
            "1M" | "1m" => Some(Timeframe::M1),
            "5M" | "5m" => Some(Timeframe::M5),
            "15M" | "15m" => Some(Timeframe::M15),
            "30M" | "30m" => Some(Timeframe::M30),
            "1H" | "1h" | "60" => Some(Timeframe::H1),
            "4H" | "4h" | "240" => Some(Timeframe::H4),
            "D" | "1D" | "1d" | "D1" => Some(Timeframe::D1),
            _ => None,
        }
    }

    pub fn to_binance_interval(&self) -> &'static str {
        match self {
            Timeframe::M1 => "1m",
            Timeframe::M5 => "5m",
            Timeframe::M15 => "15m",
            Timeframe::M30 => "30m",
            Timeframe::H1 => "1h",
            Timeframe::H4 => "4h",
            Timeframe::D1 => "1d",
        }
    }
}

impl Serialize for Timeframe {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(match self {
            Timeframe::M1 => "1M",
            Timeframe::M5 => "5M",
            Timeframe::M15 => "15M",
            Timeframe::M30 => "30M",
            Timeframe::H1 => "1H",
            Timeframe::H4 => "4H",
            Timeframe::D1 => "D",
        })
    }
}

impl<'de> Deserialize<'de> for Timeframe {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Timeframe::from_str(&s).ok_or_else(|| serde::de::Error::custom(format!("invalid timeframe: {}", s)))
    }
}

/// Market type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MarketType {
    Spot,
    Futures,
}

/// Exchange identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Exchange {
    Binance,
    /// TradingView connector (PyPI tv_connector servisi üzerinden)
    TradingView,
    // Future: Bybit, OKX, etc.
}

/// Signal direction
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SignalType {
    Buy,
    Sell,
    GetReadyBuy,
    GetReadySell,
    ChochBuy,
    ChochSell,
    BosBuy,
    BosSell,
    Liquidity,
    BullishDivergence,
    BearishDivergence,
}

/// Trade signal output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Signal {
    pub signal_type: SignalType,
    pub price: f64,
    pub timestamp: i64,
    pub timeframe: Timeframe,
    pub take_profit: Option<f64>,
    pub stop_loss: Option<f64>,
    pub confidence: f64,
    pub trend_strength: f64,
    pub metadata: serde_json::Value,
}

/// Trend / momentum / risk breakdown used by T/D/Q style setups.
///
/// All scores are in the 0.0–1.0 range; points are integer buckets for UI
/// (for example 0–4, 0–3, 0–3 → 1–10 overall).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrendMomentumRiskScores {
    /// Normalized trend score (0.0–1.0)
    pub trend_score: f64,
    /// Normalized momentum score (0.0–1.0)
    pub momentum_score: f64,
    /// Normalized risk/reward score (0.0–1.0)
    pub rr_score: f64,
    /// Combined score (0.0–1.0) before scaling to points
    pub overall_score: f64,
    /// Trend sub-score points (typically 0–4)
    pub trend_points: u8,
    /// Momentum sub-score points (typically 0–3)
    pub momentum_points: u8,
    /// Risk/Reward sub-score points (typically 0–3)
    pub rr_points: u8,
    /// Overall strength points (1–10)
    pub strength_points: u8,
}

/// Position-level metrics that can be shared between D/T/Q style views.
///
/// This struct intentionally stays generic and English-only; web/CLI layers can
/// map these fields to localized labels (Durum, Yerel Trend, etc.).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionMetrics {
    /// Symbol (e.g. ETHUSDT, XU100)
    pub symbol: String,
    /// Primary timeframe used for the analysis
    pub timeframe: Timeframe,
    /// Direction of the position, if any (Buy / Sell)
    pub side: Option<SignalType>,
    /// Local trend direction on the chart timeframe (-1, 0, +1)
    pub local_trend: i32,
    /// Higher timeframe/global trend direction (-1, 0, +1)
    pub global_trend: i32,
    /// Position state: "Long", "Short" or "Flat"
    pub position_state: String,
    /// Market regime label (e.g. "Breakout", "Range", "Pullback")
    pub market_mode: String,
    /// Volatility as percentage (ATR / price * 100)
    pub volatility_pct: f64,
    /// Short-term momentum (e.g. ROC over a small window)
    pub momentum_short: f64,
    /// Longer-term momentum
    pub momentum_long: f64,
    /// Theoretical or actual entry price used for metrics
    pub entry_price: f64,
    /// Initial stop loss level used for metrics
    pub stop_loss_initial: f64,
    /// Initial take profit level used for metrics
    pub take_profit_initial: f64,
    /// Active trailing stop level (if any; NaN if not available)
    pub stop_trail_active: f64,
    /// Dynamic take profit level (if any; NaN if not available)
    pub take_profit_dynamic: f64,
    /// Realized or theoretical risk/reward ratio for the position
    pub rr: f64,
    /// Trend/Momentum/RR breakdown and overall strength points (TSK)
    pub tmr_scores: TrendMomentumRiskScores,
    /// True if trend exhaustion conditions are detected
    pub trend_exhaustion: bool,
    /// True if structure shift (BOS/CHOCH style) is detected
    pub structure_shift: bool,
}

/// Q-Setup / Q-Analiz erken uyarı yapısı
/// Tek mal - tek hedef - tek stop yaklaşımını taşır.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QSetup {
    /// Sembol (ör. ETHUSDT, XU100)
    pub symbol: String,
    /// Zaman dilimi (5M, 1H, vb.)
    pub timeframe: Timeframe,
    /// Yön (Buy / Sell)
    pub side: SignalType,
    /// Önerilen giriş fiyatı (tek nokta; pratikte entry_zone içinde)
    pub entry: f64,
    /// Giriş bölgesi [min, max] – pivot + α·ATR .. β·ATR
    pub entry_zone: (f64, f64),
    /// Önerilen stop seviyesi
    pub stop_loss: f64,
    /// Önerilen hedef seviyesi
    pub take_profit: f64,
    /// 0–100 arası Q güven skoru
    pub q_score: f64,
    /// Setup'ın tamamlanması için Fibo-zaman penceresi (bar) [T_start, T_end]
    pub time_window_bars: (u32, u32),
    /// Setup'ın istatistiksel olarak tamamlanmasının beklendiği bar sayısı (özet)
    pub expected_bars: u32,
    /// Q-RADAR tarafından erken tetiklendi mi
    #[serde(default)]
    pub radar_early: bool,
}

/// Q-RADAR erken uyarı sinyali – Q-Setup'tan önce oluşan, zaman penceresi odaklı uyarı.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QRadarSignal {
    /// Sembol (ör. ETHUSDT, XU100)
    pub symbol: String,
    /// Zaman dilimi
    pub timeframe: Timeframe,
    /// Yön (Buy / Sell)
    pub side: SignalType,
    /// 0–1 arası güven skoru
    pub confidence: f64,
    /// Beklenen tamamlanma penceresi (bar cinsinden) [min, max]
    pub expected_window_bars: (u32, u32),
    /// İzlenecek / referans giriş fiyatı (son kapanış)
    pub reference_price: f64,
    /// Tahmini stop loss (pivot veya ATR ile; Q-Setup çıkınca kesinleşir)
    pub suggested_sl: Option<f64>,
}

/// Poz Koruma / çıkış uyarısı – kar içindeyken zorunlu koruma sinyali.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtectSignal {
    /// Sembol
    pub symbol: String,
    /// Zaman dilimi
    pub timeframe: Timeframe,
    /// Poz koruma sebebi (ör. "TRAILING_PROFIT", "STRUCTURE_BREAK")
    pub reason: String,
    /// Tetiklenecek fiyat seviyesi
    pub trigger_price: f64,
    /// Referans giriş fiyatı (R hesabı için)
    pub entry_price: f64,
    /// Kilitlenecek kâr (R cinsinden)
    pub locked_r: f64,
}
