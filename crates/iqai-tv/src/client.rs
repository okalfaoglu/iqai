//! TradingView connector: varsayılan saf Rust (WebSocket). İsteğe bağlı HTTP veya subprocess.

use async_trait::async_trait;
use iqai_core::exchange::{
    ExchangeConnector, ExchangeError, ExchangeResult, OrderResponse, OrderSide,
};
use iqai_core::types::{Candle, Exchange, MarketType, Timeframe};
use iqai_core::AppConfig;
use reqwest::Client;
use serde::Deserialize;
use std::process::Command;

use tradingview::history;
use tradingview::live::models::DataServer;
use tradingview::prelude::{Interval as TvInterval, OHLCV};
use tradingview::UserCookies;

use crate::native;

#[derive(Debug, Deserialize)]
struct HistoryResponse {
    candles: Vec<TvCandle>,
}

#[derive(Debug, Deserialize)]
struct TvCandle {
    time: i64,
    open: f64,
    high: f64,
    low: f64,
    close: f64,
    volume: f64,
}

fn timeframe_to_tv_interval(tf: Timeframe) -> &'static str {
    match tf {
        Timeframe::M1 => "1",
        Timeframe::M5 => "5",
        Timeframe::M15 => "15",
        Timeframe::M30 => "30",
        Timeframe::H1 => "1H",
        Timeframe::H4 => "4H",
        Timeframe::D1 => "1D",
    }
}

/// Backend: Native (Rust WebSocket), HTTP (uvicorn) veya subprocess (Python script).
pub enum TvBackend {
    /// Saf Rust – TradingView WebSocket; Python/8765 gerekmez.
    Native,
    Http { base_url: String },
    Subprocess { python: String, script_path: String },
    /// tradingview-rs: TV hesabı ile (TV_AUTH_TOKEN) resmi olmayan client
    TradingViewRs,
}

/// TradingView connector. Varsayılan Native (Rust). İsteğe bağlı HTTP veya subprocess.
pub struct TvConnectorClient {
    backend: TvBackend,
    exchange: String,
}

impl TvConnectorClient {
    fn load_tv_auth_token() -> Option<String> {
        // 1) Önce ortam değişkeni
        if let Ok(t) = std::env::var("TV_AUTH_TOKEN") {
            if !t.trim().is_empty() {
                eprintln!("[TV] Auto backend: TV_AUTH_TOKEN ortam değişkeninden bulundu (kısaltılmış)");
                return Some(t);
            }
        }
        // 2) config.json içinden tradingview_auth_token (isteğe bağlı)
        if let Some(cfg) = AppConfig::load() {
            if let Some(t) = cfg.tradingview_auth_token {
                if !t.trim().is_empty() {
                    eprintln!("[TV] Auto backend: tradingview_auth_token config.json'dan yüklendi (kısaltılmış)");
                    return Some(t);
                }
            }
        }
        None
    }
    /// Native Rust WebSocket – TV'den doğrudan veri (Python/HTTP yok).
    pub fn native(exchange: impl Into<String>) -> Self {
        Self {
            backend: TvBackend::Native,
            exchange: exchange.into(),
        }
    }

    /// Otomatik seçim: TV_AUTH_TOKEN varsa tradingview-rs, yoksa Native.
    pub fn auto(exchange: impl Into<String>) -> Self {
        let exchange = exchange.into();
        if let Some(token) = Self::load_tv_auth_token() {
            // tradingview-rs, env üzerinden token beklediği için eksikse set et
            if std::env::var("TV_AUTH_TOKEN").is_err() {
                std::env::set_var("TV_AUTH_TOKEN", &token);
                eprintln!("[TV] Auto backend: TV_AUTH_TOKEN env'e config/token'dan yazıldı");
            }
            eprintln!("[TV] Auto backend: TradingViewRs seçildi (exchange={})", exchange);
            Self {
                backend: TvBackend::TradingViewRs,
                exchange,
            }
        } else if let Some(cfg) = AppConfig::load() {
            // Token yok ama config'de username/password varsa tradingview-rs ile login dene (login, fetch çağrısında yapılacak)
            if cfg.tv_username.as_deref().map(|s| !s.trim().is_empty()).unwrap_or(false)
                && cfg.tv_password.as_deref().map(|s| !s.trim().is_empty()).unwrap_or(false)
            {
                eprintln!(
                    "[TV] Auto backend: config'te tv_username/tv_password var, TradingViewRs seçildi (exchange={})",
                    exchange
                );
                Self {
                    backend: TvBackend::TradingViewRs,
                    exchange,
                }
            } else {
                eprintln!(
                    "[TV] Auto backend: token ve geçerli tv_username/tv_password yok, Native backend'e düşüldü (exchange={})",
                    exchange
                );
                Self::native(exchange)
            }
        } else {
            eprintln!("[TV] Auto backend: token bulunamadı, Native backend'e düşüldü (exchange={})", exchange);
            Self::native(exchange)
        }
    }

    /// HTTP: uvicorn main:app --port 8765 çalışıyor olmalı.
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            backend: TvBackend::Http {
                base_url: base_url.into(),
            },
            exchange: "BINANCE".to_string(),
        }
    }

    pub fn with_exchange(base_url: impl Into<String>, exchange: impl Into<String>) -> Self {
        Self {
            backend: TvBackend::Http {
                base_url: base_url.into(),
            },
            exchange: exchange.into(),
        }
    }

    /// Subprocess: uvicorn yok. `python script_path SYMBOL EXCHANGE INTERVAL N_BARS` çalıştırılır.
    pub fn subprocess(
        python: impl Into<String>,
        script_path: impl Into<String>,
        exchange: impl Into<String>,
    ) -> Self {
        Self {
            backend: TvBackend::Subprocess {
                python: python.into(),
                script_path: script_path.into(),
            },
            exchange: exchange.into(),
        }
    }

    async fn fetch_klines_http(
        client: &Client,
        base_url: &str,
        exchange: &str,
        symbol: &str,
        interval: Timeframe,
        limit: u32,
    ) -> ExchangeResult<Vec<Candle>> {
        let base = base_url.trim_end_matches('/');
        let interval_str = timeframe_to_tv_interval(interval);
        let url = format!(
            "{}/history?symbol={}&exchange={}&interval={}&n_bars={}",
            base, symbol, exchange, interval_str, limit.min(5000)
        );
        let resp = client
            .get(&url)
            .send()
            .await
            .map_err(|e| ExchangeError::Http(e.to_string()))?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(ExchangeError::Api(format!("{}: {}", status, body)));
        }
        let data: HistoryResponse = resp
            .json()
            .await
            .map_err(|e| ExchangeError::Http(e.to_string()))?;
        Ok(data
            .candles
            .into_iter()
            .map(|c| Candle {
                time: c.time,
                open: c.open,
                high: c.high,
                low: c.low,
                close: c.close,
                volume: c.volume,
            })
            .collect())
    }

    fn fetch_klines_subprocess(
        python: &str,
        script_path: &str,
        exchange: &str,
        symbol: &str,
        interval: Timeframe,
        limit: u32,
    ) -> ExchangeResult<Vec<Candle>> {
        let interval_str = timeframe_to_tv_interval(interval);
        let out = Command::new(python)
            .arg(script_path)
            .arg(symbol)
            .arg(exchange)
            .arg(interval_str)
            .arg(limit.min(5000).to_string())
            .output()
            .map_err(|e| ExchangeError::Http(e.to_string()))?;
        if !out.status.success() {
            let stderr = String::from_utf8_lossy(&out.stderr);
            return Err(ExchangeError::Api(format!("script error: {}", stderr)));
        }
        let data: HistoryResponse = serde_json::from_slice(&out.stdout)
            .map_err(|e| ExchangeError::Api(format!("parse: {}", e)))?;
        Ok(data
            .candles
            .into_iter()
            .map(|c| Candle {
                time: c.time,
                open: c.open,
                high: c.high,
                low: c.low,
                close: c.close,
                volume: c.volume,
            })
            .collect())
    }

    fn map_timeframe_to_tv_interval(tf: Timeframe) -> TvInterval {
        match tf {
            Timeframe::M1 => TvInterval::OneMinute,
            Timeframe::M5 => TvInterval::FiveMinutes,
            Timeframe::M15 => TvInterval::FifteenMinutes,
            Timeframe::M30 => TvInterval::ThirtyMinutes,
            Timeframe::H1 => TvInterval::OneHour,
            Timeframe::H4 => TvInterval::FourHours,
            Timeframe::D1 => TvInterval::OneDay,
        }
    }

    /// Futures sembollerini TradingView formatına dönüştür (ETHUSDT27H2026 → ETHUSDT.P, sadece symbol kısmı).
    fn format_symbol_for_tradingview(symbol: &str, exchange: &str) -> String {
        let sym = symbol.trim();
        if sym.contains(':') {
            // Kullanıcı zaten EXCHANGE:SYMBOL formatı girdiyse sadece sembol kısmını döndür (sağ taraf).
            if let Some((_, right)) = sym.split_once(':') {
                return right.to_string();
            }
            return sym.to_string();
        }
        let base = if let Some(off) = sym.find("USDT") {
            let after = sym.get(off + 4..).unwrap_or("");
            if !after.is_empty() && after.chars().any(|c| c.is_ascii_digit()) {
                sym.get(..off + 4).unwrap_or(sym).to_string()
            } else {
                sym.to_string()
            }
        } else {
            sym.to_string()
        };
        if exchange.eq_ignore_ascii_case("BINANCE") && base.contains("USDT") && !base.ends_with(".P") {
            format!("{}.P", base)
        } else {
            base
        }
    }

    async fn fetch_klines_tradingview_rs(
        exchange: &str,
        symbol: &str,
        interval: Timeframe,
        limit: u32,
    ) -> ExchangeResult<Vec<Candle>> {
        // Gerekirse login olup TV_AUTH_TOKEN üret
        if std::env::var("TV_AUTH_TOKEN").is_err() {
            eprintln!("[TV] tradingview-rs: TV_AUTH_TOKEN yok, config ile login deneniyor...");
            if let Some(cfg) = AppConfig::load() {
                if let (Some(username), Some(password)) = (cfg.tv_username, cfg.tv_password) {
                    let totp = cfg.tv_totp_secret;
                    let user = UserCookies::default()
                        .login(&username, &password, totp.as_deref())
                        .await
                        .map_err(|e| ExchangeError::Api(format!("TradingView login error: {e}")))?;
                    std::env::set_var("TV_AUTH_TOKEN", &user.auth_token);
                    eprintln!("[TV] tradingview-rs: login başarılı, TV_AUTH_TOKEN set edildi (username={})", username);
                } else {
                    eprintln!("[TV] tradingview-rs: config'de tv_username/tv_password eksik, login yapılamadı");
                }
            } else {
                eprintln!("[TV] tradingview-rs: AppConfig.load() başarısız, login yapılamadı");
            }
        }

        let tv_interval = Self::map_timeframe_to_tv_interval(interval);
        let tv_symbol = Self::format_symbol_for_tradingview(symbol, exchange);

        eprintln!(
            "[TV] tradingview-rs: history.single.retrieve() çağrılıyor symbol={} exchange={} interval={:?} limit={}",
            tv_symbol, exchange, tv_interval, limit
        );

        let builder = history::single::retrieve()
            .symbol(&tv_symbol)
            .exchange(exchange)
            .interval(tv_interval)
            .num_bars(limit as u64)
            .server(DataServer::ProData);

        let (_info, data_points) = builder.call().await.map_err(|e| {
            eprintln!("[TV] tradingview-rs: history.single.retrieve() hata: {}", e);
            ExchangeError::Api(e.to_string())
        })?;

        let candles = data_points
            .into_iter()
            .map(|dp| Candle {
                // tradingview-rs timestamp saniye cinsinden; proje geneli ms kullanıyor
                time: dp.timestamp() * 1000,
                open: dp.open(),
                high: dp.high(),
                low: dp.low(),
                close: dp.close(),
                volume: dp.volume(),
            })
            .collect();

        Ok(candles)
    }
}

#[async_trait]
impl ExchangeConnector for TvConnectorClient {
    fn exchange(&self) -> Exchange {
        Exchange::TradingView
    }

    fn market_type(&self) -> MarketType {
        MarketType::Futures
    }

    async fn fetch_klines(
        &self,
        symbol: &str,
        interval: Timeframe,
        limit: u32,
    ) -> ExchangeResult<Vec<Candle>> {
        match &self.backend {
            TvBackend::Native => native::fetch_klines_native(
                symbol,
                &self.exchange,
                interval,
                limit,
            )
            .await
            .map_err(ExchangeError::Api),
            TvBackend::TradingViewRs => {
                Self::fetch_klines_tradingview_rs(&self.exchange, symbol, interval, limit).await
            }
            TvBackend::Http { base_url } => {
                let client = Client::new();
                Self::fetch_klines_http(
                    &client,
                    base_url,
                    &self.exchange,
                    symbol,
                    interval,
                    limit,
                )
                .await
            }
            TvBackend::Subprocess { python, script_path } => tokio::task::spawn_blocking({
                let python = python.clone();
                let script_path = script_path.clone();
                let exchange = self.exchange.clone();
                let symbol = symbol.to_string();
                move || {
                    Self::fetch_klines_subprocess(
                        &python,
                        &script_path,
                        &exchange,
                        &symbol,
                        interval,
                        limit,
                    )
                }
            })
            .await
            .map_err(|e| ExchangeError::Http(e.to_string()))?
            .map_err(|e| e),
        }
    }

    async fn place_market_order(
        &self,
        _symbol: &str,
        _side: OrderSide,
        _quantity: f64,
    ) -> ExchangeResult<OrderResponse> {
        Err(ExchangeError::Api(
            "TradingView connector is read-only (data only)".to_string(),
        ))
    }

    async fn get_balance(&self, _asset: &str) -> ExchangeResult<f64> {
        Err(ExchangeError::Api(
            "TradingView connector is read-only (data only)".to_string(),
        ))
    }
}
