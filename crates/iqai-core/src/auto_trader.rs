//! Auto-Trader: Sinyal → Pozisyon Boyutu → Emir → TradeManager → Çıkış
//!
//! Üç çalışma modu:
//!   **live** – gerçek veri + gerçek Binance emri + SQLite DB kaydı
//!   **dry**  – gerçek veri + simüle emir (borsa çağrılmaz) + SQLite DB kaydı
//!   **paper** – gerçek veri + simüle emir, DB kaydı yok (geçici test)

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::app_config::TradingConfig;
use crate::config::Config;
use crate::exchange::{ExchangeConnector, OrderSide};
use crate::trade_db::TradeDb;
use crate::trade_manager::{calculate_position_size, Position, PositionSide, TradeAction, TradeManager};
use crate::types::{Candle, QSetup, SignalType, Timeframe};

/// Kapanışta kayma: long için fiyat aşağı, short için yukarı (olumsuz fill).
fn apply_slippage(price: f64, is_long: bool, slippage_bps: u32) -> f64 {
    if slippage_bps == 0 {
        return price;
    }
    let pct = slippage_bps as f64 / 10_000.0;
    if is_long {
        price * (1.0 - pct)
    } else {
        price * (1.0 + pct)
    }
}

// ──────────────────────────── Enums / structs ────────────────────────────

/// Çalışma modu
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TradingMode {
    Live,
    Dry,
    Paper,
}

impl TradingMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Live => "live",
            Self::Dry => "dry",
            Self::Paper => "paper",
        }
    }
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "live" => Self::Live,
            "dry" => Self::Dry,
            _ => Self::Paper,
        }
    }
    pub fn sends_real_orders(&self) -> bool {
        matches!(self, Self::Live)
    }
    pub fn writes_db(&self) -> bool {
        matches!(self, Self::Live | Self::Dry)
    }
}

impl std::fmt::Display for TradingMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Sinyal kaynağı türü
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SignalSource {
    QSetup,
    ElliottW3,
    ElliottW5,
    ZigzagC,
    TriangleE,
    FakeBreakout,
}

impl std::fmt::Display for SignalSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::QSetup => write!(f, "Q-Setup"),
            Self::ElliottW3 => write!(f, "Elliott W3"),
            Self::ElliottW5 => write!(f, "Elliott W5"),
            Self::ZigzagC => write!(f, "Zigzag C"),
            Self::TriangleE => write!(f, "Triangle E"),
            Self::FakeBreakout => write!(f, "Fake Breakout"),
        }
    }
}

impl SignalSource {
    /// DB'de kayıtlı source string'inden dönüştür (recovery için).
    pub fn from_db_source(s: &str) -> Self {
        match s {
            "Elliott W3" => Self::ElliottW3,
            "Elliott W5" => Self::ElliottW5,
            "Zigzag C" => Self::ZigzagC,
            "Triangle E" => Self::TriangleE,
            "Fake Breakout" => Self::FakeBreakout,
            _ => Self::QSetup,
        }
    }
}

/// Auto-Trader'a gelen birleşik sinyal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeSignal {
    pub source: SignalSource,
    pub symbol: String,
    pub timeframe: Timeframe,
    pub is_long: bool,
    pub entry: f64,
    pub stop_loss: f64,
    pub take_profit: f64,
    pub score: f64,
    pub rr: f64,
    pub timestamp: i64,
}

/// Yönetilen açık pozisyon
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManagedPosition {
    pub position: Position,
    pub signal: TradeSignal,
    pub order_id: String,
    pub opened_at: i64,
    pub realized_pnl: f64,
    /// SQLite positions.id (DB modlarda dolu)
    pub db_id: Option<i64>,
}

/// Auto-Trader sonuç logu
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeLog {
    pub timestamp: i64,
    pub symbol: String,
    pub source: SignalSource,
    pub side: String,
    pub entry: f64,
    pub exit: f64,
    pub quantity: f64,
    pub pnl: f64,
    pub pnl_r: f64,
    pub reason: String,
}

/// AutoTrader'dan yayılan bildirim olayları – Notifier tarafından tüketilir.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TradeEvent {
    /// Yeni sinyal alındı (kabul/red)
    SignalReceived {
        signal: TradeSignal,
        accepted: bool,
        reason: String,
    },
    /// Pozisyon açıldı
    PositionOpened {
        signal: TradeSignal,
        quantity: f64,
        avg_price: f64,
        mode: TradingMode,
    },
    /// Pozisyon kapatıldı (SL/TP/trailing)
    PositionClosed {
        symbol: String,
        side: String,
        entry: f64,
        exit: f64,
        quantity: f64,
        pnl: f64,
        pnl_r: f64,
        reason: String,
        source: SignalSource,
        mode: TradingMode,
    },
    /// Kısmi kapanış
    PartialClose {
        symbol: String,
        side: String,
        pct: f64,
        price: f64,
        reason: String,
        mode: TradingMode,
    },
    /// SL güncellendi (breakeven veya trailing)
    SlUpdated {
        symbol: String,
        old_sl: f64,
        new_sl: f64,
        reason: String,
    },
    /// Günlük performans özeti
    DailySummary {
        date: String,
        summary: TradeSummary,
        mode: TradingMode,
    },
    /// Elliott Wave setup tespit edildi
    ElliottSetup {
        symbol: String,
        timeframe: Timeframe,
        source: SignalSource,
        side: String,
        entry: f64,
        stop_loss: f64,
        take_profit: f64,
        rr: f64,
    },
}

/// Trading performans özeti
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeSummary {
    pub total_trades: usize,
    pub winners: usize,
    pub losers: usize,
    pub win_rate: f64,
    pub total_pnl: f64,
    pub avg_r: f64,
    pub open_positions: usize,
}

// ──────────────────────────── Config ────────────────────────────

/// Auto-Trader'ın çalışma parametreleri (TradingConfig'den türetilir)
#[derive(Debug, Clone)]
pub struct AutoTraderConfig {
    pub enabled: bool,
    pub mode: TradingMode,
    pub risk_per_trade_pct: f64,
    pub max_positions: usize,
    pub max_leverage: f64,
    pub daily_loss_limit_pct: f64,
    pub min_q_score: f64,
    pub min_rr: f64,
    pub use_radar_filter: bool,
    pub min_radar_confidence: f64,
    /// Q-Analiz dip/tepe discrete skor filtresi (0–10). 4 = WATCH ve üstü.
    pub min_qanaliz_discrete_score: u8,
    /// Smart Money Radar skor filtresi (0–10). 4 = SM WATCH ve üstü.
    pub min_qanaliz_sm_score: u8,
    /// Fake breakout: liquidity lookback (bar)
    pub fake_breakout_lookback: usize,
    /// Fake breakout: BOS lookback (bar)
    pub fake_breakout_bos_lookback: usize,
    /// Fake breakout: min wick ratio (0..1)
    pub fake_breakout_min_wick_ratio: f64,
    /// Fake breakout: SL buffer ATR multiple
    pub fake_breakout_sl_atr_mult: f64,
    /// Fake breakout: fallback TP RR multiple
    pub fake_breakout_tp_rr: f64,
    pub db_path: Option<String>,
    /// Komisyon (basis points). Önce borsa API'sinden alınır; yoksa config (varsayılan 4).
    pub commission_bps: u32,
    /// Kapanışta kayma (basis points). Long kapanışta fiyat aşağı, short'ta yukarı uygulanır.
    pub slippage_bps: u32,
    /// true ise piyasa emri yerine limit IOC (maks kayma ile).
    pub use_limit_order: bool,
    /// Limit emirde izin verilen maks kayma, basis points (örn. 50 = %0.5).
    pub limit_slippage_bps: u32,
}

impl Default for AutoTraderConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            mode: TradingMode::Paper,
            risk_per_trade_pct: 1.0,
            max_positions: 3,
            max_leverage: 10.0,
            daily_loss_limit_pct: 3.0,
            min_q_score: 70.0,
            min_rr: 1.5,
            use_radar_filter: false,
            min_radar_confidence: 4.0,
            min_qanaliz_discrete_score: 4,
            min_qanaliz_sm_score: 4,
            fake_breakout_lookback: 40,
            fake_breakout_bos_lookback: 6,
            fake_breakout_min_wick_ratio: 0.35,
            fake_breakout_sl_atr_mult: 0.2,
            fake_breakout_tp_rr: 2.0,
            db_path: None,
            commission_bps: 4,
            slippage_bps: 0,
            use_limit_order: false,
            limit_slippage_bps: 50,
        }
    }
}

impl AutoTraderConfig {
    pub fn from_trading_config(tc: &TradingConfig) -> Self {
        Self {
            enabled: tc.enabled.unwrap_or(false),
            mode: TradingMode::from_str(tc.mode.as_deref().unwrap_or("paper")),
            risk_per_trade_pct: tc.risk_per_trade_pct.unwrap_or(1.0),
            max_positions: tc.max_positions.unwrap_or(3) as usize,
            max_leverage: tc.max_leverage.unwrap_or(10) as f64,
            daily_loss_limit_pct: tc.daily_loss_limit_pct.unwrap_or(3.0),
            min_q_score: tc.min_q_score.unwrap_or(70.0),
            min_rr: tc.min_rr.unwrap_or(1.5),
            use_radar_filter: tc.use_radar_filter.unwrap_or(false),
            min_radar_confidence: tc.min_radar_confidence.unwrap_or(4.0),
            min_qanaliz_discrete_score: tc.min_qanaliz_discrete_score.unwrap_or(4),
            min_qanaliz_sm_score: tc.min_qanaliz_sm_score.unwrap_or(4),
            fake_breakout_lookback: tc.fake_breakout_lookback.unwrap_or(40) as usize,
            fake_breakout_bos_lookback: tc.fake_breakout_bos_lookback.unwrap_or(6) as usize,
            fake_breakout_min_wick_ratio: tc.fake_breakout_min_wick_ratio.unwrap_or(0.35),
            fake_breakout_sl_atr_mult: tc.fake_breakout_sl_atr_mult.unwrap_or(0.2),
            fake_breakout_tp_rr: tc.fake_breakout_tp_rr.unwrap_or(2.0),
            db_path: tc.db_path.clone(),
            commission_bps: tc.commission_bps.unwrap_or(4),
            slippage_bps: tc.slippage_bps.unwrap_or(0),
            use_limit_order: tc.use_limit_order.unwrap_or(false),
            limit_slippage_bps: tc.limit_slippage_bps.unwrap_or(50),
        }
    }
}

// ──────────────────────────── Signal helpers ────────────────────────────

/// Q-Setup'tan TradeSignal üret
pub fn signal_from_q_setup(setup: &QSetup) -> TradeSignal {
    let is_long = matches!(setup.side, SignalType::Buy | SignalType::ChochBuy | SignalType::BosBuy);
    let entry = setup.entry;
    let sl = setup.stop_loss;
    let tp = setup.take_profit;
    let risk = (entry - sl).abs();
    let rr = if risk > 1e-10 { (tp - entry).abs() / risk } else { 0.0 };
    TradeSignal {
        source: SignalSource::QSetup,
        symbol: setup.symbol.clone(),
        timeframe: setup.timeframe,
        is_long,
        entry,
        stop_loss: sl,
        take_profit: tp,
        score: setup.q_score,
        rr,
        timestamp: chrono::Utc::now().timestamp_millis(),
    }
}

/// Elliott setup'larından TradeSignal üret
pub fn signal_from_elliott(
    source: SignalSource,
    symbol: &str,
    timeframe: Timeframe,
    is_long: bool,
    entry: f64,
    stop_loss: f64,
    take_profit: f64,
    score: f64,
) -> TradeSignal {
    let risk = (entry - stop_loss).abs();
    let rr = if risk > 1e-10 { (take_profit - entry).abs() / risk } else { 0.0 };
    TradeSignal {
        source,
        symbol: symbol.to_string(),
        timeframe,
        is_long,
        entry,
        stop_loss,
        take_profit,
        score,
        rr,
        timestamp: chrono::Utc::now().timestamp_millis(),
    }
}

// ──────────────────────────── AutoTrader ────────────────────────────

/// Otomatik trading motoru
pub struct AutoTrader {
    pub config: AutoTraderConfig,
    trade_manager: TradeManager,
    pub open_positions: HashMap<String, ManagedPosition>,
    pub trade_history: Vec<TradeLog>,
    daily_pnl: f64,
    db: Option<TradeDb>,
    /// Her tick sonunda tüketilecek bildirim olayları
    pending_events: Vec<TradeEvent>,
}

impl AutoTrader {
    pub fn new(config: AutoTraderConfig, sm_config: Config) -> Self {
        let db = if config.mode.writes_db() {
            match TradeDb::open(config.db_path.as_deref()) {
                Ok(db) => {
                    log::info!("[AutoTrader] DB açıldı (mod={})", config.mode);
                    Some(db)
                }
                Err(e) => {
                    log::error!("[AutoTrader] DB açılamadı: {}", e);
                    None
                }
            }
        } else {
            None
        };

        Self {
            config,
            trade_manager: TradeManager::new(sm_config),
            open_positions: HashMap::new(),
            trade_history: Vec::new(),
            daily_pnl: 0.0,
            db,
            pending_events: Vec::new(),
        }
    }

    /// Restart sonrası DB'den açık pozisyonları belleğe yükler (recovery).
    pub fn restore_open_positions(&mut self, rows: Vec<(i64, i64, String, String, String, String, f64, f64, f64, f64, f64, String)>) {
        for (db_id, opened_at, symbol, tf_str, source_str, side_str, entry_price, quantity, stop_loss, take_profit, current_sl, order_id) in rows {
            let Some(timeframe) = Timeframe::from_str(&tf_str) else { continue };
            let is_long = side_str.eq_ignore_ascii_case("LONG");
            let side = if is_long { PositionSide::Long } else { PositionSide::Short };
            let risk_r = (entry_price - stop_loss).abs().max(0.0001);
            let position = Position {
                side,
                entry_price,
                quantity,
                initial_sl: stop_loss,
                initial_tp: take_profit,
                current_sl,
                risk_r,
                remaining_pct: 1.0,
                breakeven_done: false,
                tp1_done: false,
                tp2_done: false,
            };
            let source = SignalSource::from_db_source(&source_str);
            let signal = TradeSignal {
                source,
                symbol: symbol.clone(),
                timeframe,
                is_long,
                entry: entry_price,
                stop_loss,
                take_profit,
                score: 0.0,
                rr: 0.0,
                timestamp: opened_at,
            };
            let managed = ManagedPosition {
                position,
                signal,
                order_id: order_id.clone(),
                opened_at,
                realized_pnl: 0.0,
                db_id: Some(db_id),
            };
            let key = position_key(&symbol, timeframe);
            self.open_positions.insert(key, managed);
        }
    }

    /// Sinyali filtrele: score, RR, pozisyon limiti, günlük kayıp limiti.
    pub fn should_take_signal(&self, signal: &TradeSignal, account_balance: f64) -> (bool, String) {
        if !self.config.enabled {
            return (false, "Auto-trader devre dışı".into());
        }
        if self.open_positions.len() >= self.config.max_positions {
            return (false, format!("Maks pozisyon limiti ({}) dolu", self.config.max_positions));
        }
        if signal.rr < self.config.min_rr {
            return (false, format!("RR {:.2} < min {:.2}", signal.rr, self.config.min_rr));
        }
        if signal.score < self.config.min_q_score {
            return (false, format!("Score {:.0} < min {:.0}", signal.score, self.config.min_q_score));
        }
        let key = position_key(&signal.symbol, signal.timeframe);
        if self.open_positions.contains_key(&key) {
            return (false, format!("{} zaten açık pozisyon var", key));
        }
        for (_, mp) in &self.open_positions {
            if mp.signal.symbol == signal.symbol && mp.signal.is_long != signal.is_long {
                return (false, format!(
                    "{} zaten {} yönünde açık — zıt sinyal reddedildi",
                    signal.symbol,
                    if mp.signal.is_long { "LONG" } else { "SHORT" },
                ));
            }
        }
        let loss_limit = account_balance * self.config.daily_loss_limit_pct / 100.0;
        if self.daily_pnl < 0.0 && self.daily_pnl.abs() >= loss_limit {
            return (false, format!("Günlük kayıp limiti ({:.2}) aşıldı", loss_limit));
        }
        (true, "OK".into())
    }

    /// Aynı sembol ve aynı timeframe için zıt yönde açık pozisyonları kapat.
    /// Farklı periyottan gelen ters sinyal mevcut pozisyonu kapatmaz (örn. 15m LONG, 5m SHORT → kapatma).
    pub async fn close_opposite_positions_for_symbol(
        &mut self,
        symbol: &str,
        signal_timeframe: Timeframe,
        signal_is_long: bool,
        current_prices: &HashMap<String, f64>,
        exchange: &dyn ExchangeConnector,
    ) -> Vec<TradeLog> {
        let keys_to_close: Vec<String> = self
            .open_positions
            .iter()
            .filter(|(_, mp)| {
                mp.signal.symbol == symbol
                    && mp.signal.timeframe == signal_timeframe
                    && mp.signal.is_long != signal_is_long
            })
            .map(|(k, _)| k.clone())
            .collect();
        let mut logs = Vec::new();
        let price = current_prices.get(symbol).copied().unwrap_or(0.0);
        for key in keys_to_close {
            if self.open_positions.contains_key(&key) {
                if let Ok(Some(entry)) = self
                    .close_position(
                        &key,
                        price,
                        "Zıt yön — yeni sinyal öncesi kapatıldı",
                        exchange,
                    )
                    .await
                {
                    logs.push(entry);
                }
            }
        }
        logs
    }

    /// Sinyali işle: pozisyon boyutu hesapla → emir gönder / simüle et → DB'ye yaz.
    /// DRY/PAPER'da `current_prices` verilirse açılış fill fiyatı o anki fiyat (son mum kapanışı) alınır; yoksa sinyal entry kullanılır.
    pub async fn process_signal(
        &mut self,
        signal: TradeSignal,
        account_balance: f64,
        exchange: &dyn ExchangeConnector,
        current_prices: Option<&HashMap<String, f64>>,
    ) -> Result<Option<ManagedPosition>, String> {
        let (mut ok, mut reason) = self.should_take_signal(&signal, account_balance);

        // DRY/PAPER: mevcut fiyat zaten TP ötesindeyse giriş yapma (geç kalmış sinyal)
        if ok && !self.config.mode.sends_real_orders() {
            if let Some(prices) = current_prices {
                let fill_price = prices.get(&signal.symbol).copied().unwrap_or(signal.entry);
                let past_tp = (signal.is_long && fill_price >= signal.take_profit)
                    || (!signal.is_long && fill_price <= signal.take_profit);
                if past_tp {
                    ok = false;
                    reason = format!("Mevcut fiyat ({:.2}) TP ötesinde (geç giriş)", fill_price);
                }
            }
        }

        // DB'ye sinyal kaydı (kabul/red fark etmez)
        let signal_db_id = if let Some(ref db) = self.db {
            db.insert_signal(&signal, ok, if ok { None } else { Some(&reason) }, self.config.mode)
                .map_err(|e| format!("DB sinyal yazma hatası: {}", e))?
        } else {
            0
        };

        if !ok {
            log::info!("[AutoTrader][{}] Sinyal reddedildi: {} — {}", self.config.mode, signal.source, reason);
            return Ok(None);
        }

        self.pending_events.push(TradeEvent::SignalReceived {
            signal: signal.clone(),
            accepted: true,
            reason: "OK".into(),
        });

        let qty = calculate_position_size(
            account_balance,
            self.config.risk_per_trade_pct,
            signal.entry,
            signal.stop_loss,
            self.config.max_leverage,
        );
        if qty <= 0.0 {
            return Err("Pozisyon boyutu hesaplanamadı".into());
        }

        let side = if signal.is_long { PositionSide::Long } else { PositionSide::Short };
        let order_side = if signal.is_long { OrderSide::Buy } else { OrderSide::Sell };
        let mode = self.config.mode;

        let (order_id, avg_price) = if mode.sends_real_orders() {
            let fill_price = current_prices
                .and_then(|m| m.get(&signal.symbol).copied())
                .unwrap_or(signal.entry);
            let limit_price = if signal.is_long {
                fill_price * (1.0 + self.config.limit_slippage_bps as f64 / 10_000.0)
            } else {
                fill_price * (1.0 - self.config.limit_slippage_bps as f64 / 10_000.0)
            };
            let order_result = if self.config.use_limit_order {
                exchange
                    .place_limit_order_ioc(&signal.symbol, order_side, qty, limit_price)
                    .await
            } else {
                exchange.place_market_order(&signal.symbol, order_side, qty).await
            };
            match order_result {
                Ok(resp) => {
                    log::info!(
                        "[AutoTrader][LIVE] Order {} filled: {:.6} @ {:.2}",
                        resp.order_id, resp.executed_qty, resp.avg_price,
                    );
                    (resp.order_id, if resp.avg_price > 0.0 { resp.avg_price } else { signal.entry })
                }
                Err(e) => {
                    log::error!("[AutoTrader] Emir hatası: {}", e);
                    return Err(format!("Emir hatası: {}", e));
                }
            }
        } else {
            // DRY / PAPER: simüle emir — fill fiyatı o anki piyasa (son mum kapanışı) veya sinyal entry
            let fill_price = current_prices
                .and_then(|m| m.get(&signal.symbol).copied())
                .unwrap_or(signal.entry);
            log::info!(
                "[AutoTrader][{}] {} {} qty={:.6} @ {:.2} SL={:.2} TP={:.2}",
                mode,
                if signal.is_long { "LONG" } else { "SHORT" },
                signal.symbol, qty, fill_price, signal.stop_loss, signal.take_profit,
            );
            (format!("{}-{}", mode.as_str().to_uppercase(), signal.timestamp), fill_price)
        };

        let position = self.trade_manager.create_position(side, avg_price, qty, signal.stop_loss, signal.take_profit);

        let mut managed = ManagedPosition {
            position,
            signal: signal.clone(),
            order_id,
            opened_at: signal.timestamp,
            realized_pnl: 0.0,
            db_id: None,
        };

        // DB'ye pozisyon kaydı
        if let Some(ref db) = self.db {
            match db.insert_position(signal_db_id, &managed, mode) {
                Ok(id) => {
                    managed.db_id = Some(id);
                    // Analiz link'i oluştur – mümkünse en yakın Q-RADAR event'i ve snapshot bilgisiyle.
                    let tf_str = signal.timeframe.to_binance_interval();
                    // H4 gibi TF'ler için pencereler TF'e göre ayarlanabilir; şimdilik 1 saat (ms).
                    let window_ms: i64 = 60 * 60 * 1000;
                    let q_event_id = db
                        .find_recent_q_event_for(&signal.symbol, tf_str, signal.timestamp, window_ms)
                        .ok()
                        .flatten()
                        .map(|e| e.id);
                    // Snapshot header (symbol+tf son state).
                    let snapshot = db
                        .get_analysis_snapshots(Some(&signal.symbol))
                        .ok()
                        .and_then(|rows| {
                            rows.into_iter()
                                .find(|r| r.timeframe == tf_str)
                                .map(|r| (Some(r.symbol), Some(r.timeframe), Some(r.updated_at)))
                        })
                        .unwrap_or((None, None, None));
                    let (snap_sym, snap_tf, snap_ts) = snapshot;
                    if let Err(e) = db.insert_trade_analysis_link(
                        id,
                        signal_db_id,
                        &signal.symbol,
                        tf_str,
                        q_event_id,
                        snap_sym.as_deref(),
                        snap_tf.as_deref(),
                        snap_ts,
                        mode.as_str(),
                    ) {
                        log::error!("[AutoTrader] trade_analysis_links insert hatası: {}", e);
                    }
                }
                Err(e) => log::error!("[AutoTrader] DB pozisyon yazma hatası: {}", e),
            }
        }

        let key = position_key(&signal.symbol, signal.timeframe);
        self.open_positions.insert(key, managed.clone());

        self.pending_events.push(TradeEvent::PositionOpened {
            signal: signal.clone(),
            quantity: qty,
            avg_price,
            mode,
        });

        log::info!(
            "[AutoTrader][{}] Pozisyon açıldı: {} {} {:.6} @ {:.2} | RR={:.2} Score={:.0}",
            mode, signal.source, signal.symbol, qty, avg_price, signal.rr, signal.score,
        );

        Ok(Some(managed))
    }

    /// Her yeni mumda açık pozisyonları değerlendir.
    /// `skip_keys`: bu tick'te yeni açılan pozisyonlar; look-ahead bias önlemek için aynı bar'da TP/SL değerlendirilmez.
    pub fn tick_positions(
        &mut self,
        current_prices: &HashMap<String, f64>,
        candles_map: &HashMap<String, Vec<Candle>>,
        skip_keys: Option<&HashSet<String>>,
    ) -> Vec<(String, TradeAction)> {
        let mut actions = Vec::new();
        let mut sl_events = Vec::new();

        for (key, managed) in self.open_positions.iter_mut() {
            if let Some(skip) = skip_keys {
                if skip.contains(key) {
                    continue;
                }
            }
            let symbol = &managed.signal.symbol;
            let price = match current_prices.get(symbol.as_str()) {
                Some(&p) => p,
                None => continue,
            };
            // Pozisyon timeframe'ine göre mum seti (sembol_tf anahtarı)
            let candles = candles_map.get(key.as_str()).map(|v| v.as_slice()).unwrap_or(&[]);

            let old_sl = managed.position.current_sl;
            let action = self.trade_manager.evaluate(&mut managed.position, price, candles);
            self.trade_manager.apply_action(&mut managed.position, &action);

            match &action {
                TradeAction::None => {}
                TradeAction::UpdateTrailingStop { new_sl } => {
                    if let (Some(ref db), Some(db_id)) = (&self.db, managed.db_id) {
                        let _ = db.update_position_sl(db_id, *new_sl);
                    }
                    sl_events.push(TradeEvent::SlUpdated {
                        symbol: symbol.clone(),
                        old_sl,
                        new_sl: *new_sl,
                        reason: "Trailing Stop".into(),
                    });
                    actions.push((key.clone(), action));
                }
                TradeAction::MoveSlToBreakeven => {
                    if let (Some(ref db), Some(db_id)) = (&self.db, managed.db_id) {
                        let _ = db.update_position_sl(db_id, managed.position.entry_price);
                    }
                    sl_events.push(TradeEvent::SlUpdated {
                        symbol: symbol.clone(),
                        old_sl,
                        new_sl: managed.position.entry_price,
                        reason: "Breakeven".into(),
                    });
                    actions.push((key.clone(), action));
                }
                _ => actions.push((key.clone(), action)),
            }
        }

        self.pending_events.extend(sl_events);
        actions
    }

    /// Pozisyonu kapat, DB'ye yaz.
    pub async fn close_position(
        &mut self,
        key: &str,
        current_price: f64,
        reason: &str,
        exchange: &dyn ExchangeConnector,
    ) -> Result<Option<TradeLog>, String> {
        let managed = match self.open_positions.remove(key) {
            Some(m) => m,
            None => return Ok(None),
        };

        let close_side = if managed.signal.is_long { OrderSide::Sell } else { OrderSide::Buy };
        let close_qty = managed.position.quantity * managed.position.remaining_pct;

        if self.config.mode.sends_real_orders() && close_qty > 0.0 {
            let limit_price = if managed.signal.is_long {
                current_price * (1.0 - self.config.limit_slippage_bps as f64 / 10_000.0)
            } else {
                current_price * (1.0 + self.config.limit_slippage_bps as f64 / 10_000.0)
            };
            let order_result = if self.config.use_limit_order {
                exchange
                    .place_limit_order_ioc(&managed.signal.symbol, close_side, close_qty, limit_price)
                    .await
            } else {
                exchange
                    .place_market_order(&managed.signal.symbol, close_side, close_qty)
                    .await
            };
            match order_result {
                Ok(resp) => log::info!("[AutoTrader][LIVE] Close {}: {:.6} @ {:.2}", resp.order_id, resp.executed_qty, resp.avg_price),
                Err(e) => {
                    log::error!("[AutoTrader] Kapanış emir hatası: {}", e);
                    return Err(format!("Kapanış emir hatası: {}", e));
                }
            }
        }

        let commission_bps = exchange
            .get_commission_bps(&managed.signal.symbol)
            .await
            .unwrap_or(self.config.commission_bps);
        let effective_exit = apply_slippage(current_price, managed.signal.is_long, self.config.slippage_bps);
        let pnl_gross = if managed.signal.is_long {
            (effective_exit - managed.position.entry_price) * close_qty
        } else {
            (managed.position.entry_price - effective_exit) * close_qty
        };
        let notional_open = managed.position.entry_price * close_qty;
        let notional_close = effective_exit * close_qty;
        let fee = (notional_open + notional_close) * (commission_bps as f64 / 10_000.0);
        let pnl = pnl_gross - fee;
        let pnl_r = pnl / (managed.position.risk_r * close_qty).max(1e-10);
        self.daily_pnl += pnl;

        let trade_log = TradeLog {
            timestamp: chrono::Utc::now().timestamp_millis(),
            symbol: managed.signal.symbol.clone(),
            source: managed.signal.source,
            side: if managed.signal.is_long { "LONG".into() } else { "SHORT".into() },
            entry: managed.position.entry_price,
            exit: current_price,
            quantity: close_qty,
            pnl,
            pnl_r,
            reason: reason.to_string(),
        };

        // DB: pozisyonu kapat + trade log yaz
        if let Some(ref db) = self.db {
            if let Some(db_id) = managed.db_id {
                let _ = db.close_position(db_id, current_price, pnl, pnl_r, reason);

                // Basit outcome kaydı: full-trade performansı
                if let Ok(links) = db.get_trade_analysis_links_by_position(db_id) {
                    if let Some(link) = links.first() {
                        let entry = managed.position.entry_price;
                        let side_long = managed.signal.is_long;
                        let ret_pct = if entry.abs() > 0.0 {
                            if side_long {
                                (current_price - entry) / entry * 100.0
                            } else {
                                (entry - current_price) / entry * 100.0
                            }
                        } else {
                            0.0
                        };
                        let quality = if pnl_r >= 1.0 {
                            Some("win")
                        } else if pnl_r <= -1.0 {
                            Some("clear_fail")
                        } else {
                            Some("noisy")
                        };
                        let direction_str = if side_long { "LONG" } else { "SHORT" };
                        let _ = db.insert_analysis_outcome(
                            link.q_event_id.unwrap_or(0),
                            &link.symbol,
                            &link.timeframe,
                            direction_str,
                            "N/A",
                            entry,
                            0,           // horizon_bars: full trade
                            ret_pct,
                            None,
                            None,
                            None,
                            None,
                            quality,
                            self.config.mode.as_str(),
                        );
                    }
                }
            }
            let _ = db.insert_trade_log(&trade_log, self.config.mode);
        }

        self.pending_events.push(TradeEvent::PositionClosed {
            symbol: managed.signal.symbol.clone(),
            side: trade_log.side.clone(),
            entry: managed.position.entry_price,
            exit: current_price,
            quantity: close_qty,
            pnl,
            pnl_r,
            reason: reason.to_string(),
            source: managed.signal.source,
            mode: self.config.mode,
        });

        log::info!(
            "[AutoTrader][{}] Kapatıldı: {} {} PnL={:.2} ({:.2}R) — {}",
            self.config.mode, managed.signal.symbol, trade_log.side, pnl, pnl_r, reason,
        );

        self.trade_history.push(trade_log.clone());
        Ok(Some(trade_log))
    }

    /// Kısmi kapanış.
    pub async fn partial_close(
        &mut self,
        key: &str,
        pct: f64,
        current_price: f64,
        reason: &str,
        exchange: &dyn ExchangeConnector,
    ) -> Result<(), String> {
        let managed = match self.open_positions.get(key) {
            Some(m) => m,
            None => return Ok(()),
        };

        let close_qty = managed.position.quantity * pct;
        let close_side = if managed.signal.is_long { OrderSide::Sell } else { OrderSide::Buy };

        if self.config.mode.sends_real_orders() && close_qty > 0.0 {
            let limit_price = if managed.signal.is_long {
                current_price * (1.0 - self.config.limit_slippage_bps as f64 / 10_000.0)
            } else {
                current_price * (1.0 + self.config.limit_slippage_bps as f64 / 10_000.0)
            };
            let order_result = if self.config.use_limit_order {
                exchange
                    .place_limit_order_ioc(&managed.signal.symbol, close_side, close_qty, limit_price)
                    .await
            } else {
                exchange
                    .place_market_order(&managed.signal.symbol, close_side, close_qty)
                    .await
            };
            match order_result {
                Ok(resp) => log::info!("[AutoTrader][LIVE] Partial close {}: {:.6} @ {:.2}", resp.order_id, resp.executed_qty, resp.avg_price),
                Err(e) => return Err(format!("Kısmi kapanış hatası: {}", e)),
            }
        } else {
            log::info!("[AutoTrader][{}] Partial close {} {:.0}% @ {:.2} — {}", self.config.mode, key, pct * 100.0, current_price, reason);
        }

        if let Some(m) = self.open_positions.get_mut(key) {
            let commission_bps = exchange
                .get_commission_bps(&m.signal.symbol)
                .await
                .unwrap_or(self.config.commission_bps);
            let effective_exit = apply_slippage(current_price, m.signal.is_long, self.config.slippage_bps);
            let pnl_gross = if m.signal.is_long {
                (effective_exit - m.position.entry_price) * close_qty
            } else {
                (m.position.entry_price - effective_exit) * close_qty
            };
            let fee = (m.position.entry_price * close_qty + effective_exit * close_qty) * (commission_bps as f64 / 10_000.0);
            let pnl_partial = pnl_gross - fee;
            m.realized_pnl += pnl_partial;
            self.daily_pnl += pnl_partial;

            self.pending_events.push(TradeEvent::PartialClose {
                symbol: m.signal.symbol.clone(),
                side: if m.signal.is_long { "LONG".into() } else { "SHORT".into() },
                pct,
                price: current_price,
                reason: reason.to_string(),
                mode: self.config.mode,
            });
        }

        Ok(())
    }

    /// Tam döngü: sinyaller ve fiyatlarla tek tick.
    /// Bu tick'te açılan pozisyonlar aynı bar'da TP/SL ile değerlendirilmez (look-ahead bias önlemi).
    pub async fn full_tick(
        &mut self,
        signals: &[TradeSignal],
        account_balance: f64,
        current_prices: &HashMap<String, f64>,
        candles_map: &HashMap<String, Vec<Candle>>,
        exchange: &dyn ExchangeConnector,
    ) -> Vec<TradeLog> {
        let mut logs = Vec::new();
        let keys_before: HashSet<String> = self.open_positions.keys().cloned().collect();

        let leverage = self.config.max_leverage;
        for signal in signals {
            let available_balance = (account_balance - self.used_margin(leverage)).max(0.0);
            let (would_take, _reason) = self.should_take_signal(&signal, available_balance);
            // Zıt yönü sadece yeni sinyal gerçekten kabul edilecekse kapat (yoksa LONG kapatıp SHORT reddedilince gereksiz zarar)
            if would_take {
                let closed = self
                    .close_opposite_positions_for_symbol(
                        &signal.symbol,
                        signal.timeframe,
                        signal.is_long,
                        current_prices,
                        exchange,
                    )
                    .await;
                logs.extend(closed);
                let available_balance2 = (account_balance - self.used_margin(leverage)).max(0.0);
                let _ = self.process_signal(signal.clone(), available_balance2, exchange, Some(current_prices)).await;
            } else {
                let _ = self.process_signal(signal.clone(), available_balance, exchange, Some(current_prices)).await;
            }
        }

        let opened_this_tick: HashSet<String> = self
            .open_positions
            .keys()
            .cloned()
            .filter(|k| !keys_before.contains(k))
            .collect();
        // Sadece Paper modda aynı bar'da açılan pozisyonları TP/SL'dan hariç tut (backtest look-ahead önlemi).
        // Live/Dry'da mum içi SL tetiklenebilsin diye skip yapılmaz.
        let skip_keys = (self.config.mode == TradingMode::Paper).then_some(&opened_this_tick);
        let actions = self.tick_positions(current_prices, candles_map, skip_keys);

        for (key, action) in actions {
            let symbol = self.open_positions.get(&key).map(|m| m.signal.symbol.clone()).unwrap_or_default();
            let price = current_prices.get(symbol.as_str()).copied().unwrap_or(0.0);

            match &action {
                TradeAction::FullClose { reason } => {
                    if let Ok(Some(entry)) = self.close_position(&key, price, reason, exchange).await {
                        logs.push(entry);
                    }
                }
                TradeAction::PartialClose { pct, reason } => {
                    let _ = self.partial_close(&key, *pct, price, reason, exchange).await;
                }
                _ => {}
            }
        }

        logs
    }

    pub fn reset_daily_pnl(&mut self) {
        self.daily_pnl = 0.0;
    }
    pub fn daily_pnl(&self) -> f64 {
        self.daily_pnl
    }
    pub fn open_position_count(&self) -> usize {
        self.open_positions.len()
    }

    /// Açık pozisyonların kullandığı toplam margin (notional / leverage).
    /// Yeni pozisyon boyutu için kullanılabilir bakiye = toplam_bakiye - used_margin.
    pub fn used_margin(&self, leverage: f64) -> f64 {
        if leverage <= 0.0 {
            return 0.0;
        }
        self.open_positions
            .values()
            .map(|m| {
                let notional = m.position.quantity * m.position.entry_price * m.position.remaining_pct;
                notional / leverage
            })
            .sum()
    }

    pub fn mode(&self) -> TradingMode {
        self.config.mode
    }

    /// Bekleyen bildirim olaylarını al ve sıfırla. Caller bunları Notifier'a iletir.
    pub fn drain_events(&mut self) -> Vec<TradeEvent> {
        std::mem::take(&mut self.pending_events)
    }

    /// Günlük özet event'i üret.
    pub fn emit_daily_summary(&mut self) {
        let s = self.summary();
        let date = chrono::Utc::now().format("%Y-%m-%d").to_string();
        self.pending_events.push(TradeEvent::DailySummary {
            date,
            summary: s,
            mode: self.config.mode,
        });
    }

    /// Günlük özet kaydet (gece yarısı çağrılır).
    pub fn save_daily_summary(&self) {
        let s = self.summary();
        if let Some(ref db) = self.db {
            let date = chrono::Utc::now().format("%Y-%m-%d").to_string();
            let _ = db.insert_daily_summary(
                &date, s.total_trades, s.winners, s.losers, s.win_rate, s.total_pnl, s.avg_r, self.config.mode,
            );
        }
    }

    pub fn summary(&self) -> TradeSummary {
        let total_trades = self.trade_history.len();
        let winners = self.trade_history.iter().filter(|t| t.pnl > 0.0).count();
        let losers = self.trade_history.iter().filter(|t| t.pnl < 0.0).count();
        let total_pnl: f64 = self.trade_history.iter().map(|t| t.pnl).sum();
        let avg_r: f64 = if total_trades > 0 {
            self.trade_history.iter().map(|t| t.pnl_r).sum::<f64>() / total_trades as f64
        } else {
            0.0
        };
        let win_rate = if total_trades > 0 { winners as f64 / total_trades as f64 * 100.0 } else { 0.0 };
        TradeSummary { total_trades, winners, losers, win_rate, total_pnl, avg_r, open_positions: self.open_positions.len() }
    }
}

fn position_key(symbol: &str, tf: Timeframe) -> String {
    format!("{}_{}", symbol, tf.to_binance_interval())
}
