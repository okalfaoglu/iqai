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
