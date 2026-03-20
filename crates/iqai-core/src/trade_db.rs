//! SQLite tabanlı trade veritabanı.
//! live ve dry modlarda tüm sinyaller, açık/kapalı pozisyonlar ve PnL kaydedilir.

use chrono::Datelike;
use rusqlite::{params, Connection, Result as SqlResult};
use serde::Serialize;

use crate::analysis_snapshot::AnalysisSnapshot;
use crate::auto_trader::{ManagedPosition, TradeLog, TradeSignal, TradingMode};
use crate::q_radar_analysis::QRadarOpportunityAnalysis;

/// Q-Analiz tespit kaydı (DB'den okuma / web listesi).
#[derive(Debug, Clone, Serialize)]
pub struct QAnalizDetectionRecord {
    pub id: i64,
    pub symbol: String,
    pub timeframe: String,
    pub detection: String,
    pub direction: String,
    pub confidence_score: f64,
    pub early_warning_score: f64,
    pub recommendation: String,
    pub reference_price: f64,
    pub confirmation_layers: Option<String>,
    pub created_at: i64,
}

/// Event sonrası outcome kaydı (doğruluk/performans analizi için).
#[derive(Debug, Clone, Serialize)]
pub struct AnalysisOutcomeRecord {
    pub id: i64,
    pub event_id: i64,
    pub symbol: String,
    pub timeframe: String,
    pub direction: String,
    pub recommendation: String,
    pub reference_price: f64,
    pub horizon_bars: i32,
    pub return_pct: f64,
    pub mfe_pct: Option<f64>,
    pub mae_pct: Option<f64>,
    pub tp_hit: Option<bool>,
    pub sl_hit: Option<bool>,
    pub quality_label: Option<String>,
    pub mode: String,
    pub created_at: i64,
}

/// Trade ile analiz state/event'ini bağlayan link kaydı.
#[derive(Debug, Clone, Serialize)]
pub struct TradeAnalysisLink {
    pub id: i64,
    pub position_id: i64,
    pub signal_id: i64,
    pub symbol: String,
    pub timeframe: String,
    pub q_event_id: Option<i64>,
    pub snapshot_symbol: Option<String>,
    pub snapshot_timeframe: Option<String>,
    pub snapshot_updated_at: Option<i64>,
    pub mode: String,
    pub created_at: i64,
}

/// DB'den okunan analiz snapshot satırı (API / AI raporu için).
#[derive(Debug, Clone, Serialize)]
pub struct AnalysisSnapshotRecord {
    pub symbol: String,
    pub timeframe: String,
    pub updated_at: i64,
    pub detection: String,
    pub direction: String,
    pub recommendation: String,
    pub confidence_score: f64,
    pub early_warning_score: f64,
    pub reference_price: f64,
    pub confirmation_layers: Option<String>,
    pub discrete_score: Option<f64>,
    pub sm_score: Option<f64>,
    pub confluence_layers: Option<i32>,
    pub radar_confidence: Option<f64>,
    pub radar_window_min: Option<i32>,
    pub radar_window_max: Option<i32>,
    pub radar_suggested_sl: Option<f64>,
    pub dip_price: Option<f64>,
    pub dip_time: Option<i64>,
    pub bars_since_dip: Option<i32>,
    pub reversal_detected: Option<bool>,
    pub reversal_strength: Option<f64>,
    pub bounce_from_dip: Option<f64>,
    pub bounce_r: Option<f64>,
    pub spring_detected: Option<bool>,
    pub peak_price: Option<f64>,
    pub peak_time: Option<i64>,
    pub bars_since_peak: Option<i32>,
    pub peak_reversal_detected: Option<bool>,
    pub decline_strength: Option<f64>,
    pub decline_from_peak: Option<f64>,
    pub decline_r: Option<f64>,
    pub upthrust_detected: Option<bool>,
    pub mtf_support_near: Option<bool>,
    pub ltf_structure_ok: Option<bool>,
    pub fib_elliott_zone: Option<bool>,
    pub divergence_ok: Option<bool>,
    pub confluence_spring_ok: Option<bool>,
    pub rsi_zone_ok: Option<bool>,
    pub bos_ok: Option<bool>,
    pub absorption_ok: Option<bool>,
    pub rsi_14: Option<f64>,
    pub atr_14: Option<f64>,
    pub macd_line: Option<f64>,
    pub macd_signal: Option<f64>,
    pub macd_hist: Option<f64>,
    pub bb_lower: Option<f64>,
    pub bb_middle: Option<f64>,
    pub bb_upper: Option<f64>,
    pub ema_20: Option<f64>,
    pub ema_50: Option<f64>,
    pub ema_200: Option<f64>,
    pub vwap_val: Option<f64>,
    pub elliott_formation: Option<String>,
    pub elliott_type: Option<String>,
    pub elliott_in_progress: Option<bool>,
    pub elliott_validation_ok: Option<bool>,
    pub elliott_w5_t1: Option<f64>,
    pub elliott_w5_t2: Option<f64>,
    pub elliott_w5_t3: Option<f64>,
    pub classic_pattern: Option<String>,
    pub scenario_role: Option<String>,
    pub scenario_direction: Option<String>,
    pub scenario_entry: Option<f64>,
    pub scenario_stop: Option<f64>,
    pub scenario_tp1: Option<f64>,
    pub scenario_tp2: Option<f64>,
    pub scenario_tp3: Option<f64>,
    pub scenario_qscore: Option<f64>,
    pub scenario_has_radar: Option<bool>,
    pub po3_phase: Option<String>,
    pub position_state: Option<String>,
    pub market_mode: Option<String>,
    pub local_trend: Option<i32>,
    pub global_trend: Option<i32>,
    pub volatility_pct: Option<f64>,
    pub momentum_short: Option<f64>,
    pub momentum_long: Option<f64>,
    pub rr: Option<f64>,
    pub tmr_trend_points: Option<i32>,
    pub tmr_momentum_points: Option<i32>,
    pub tmr_rr_points: Option<i32>,
    pub tmr_strength_points: Option<i32>,
    pub trend_exhaustion: Option<bool>,
    pub structure_shift: Option<bool>,
    pub position_side: Option<String>,
    pub extra_json: Option<String>,
}

/// Integer (0/1) -> Option<bool>
fn opt_bool(row: &rusqlite::Row<'_>, idx: usize) -> SqlResult<Option<bool>> {
    let v: Option<i32> = row.get(idx)?;
    Ok(v.map(|x| x != 0))
}

fn map_analysis_snapshot_row(row: &rusqlite::Row<'_>) -> SqlResult<AnalysisSnapshotRecord> {
    Ok(AnalysisSnapshotRecord {
        symbol: row.get(0)?,
        timeframe: row.get(1)?,
        updated_at: row.get(2)?,
        detection: row.get(3)?,
        direction: row.get(4)?,
        recommendation: row.get(5)?,
        confidence_score: row.get(6)?,
        early_warning_score: row.get(7)?,
        reference_price: row.get(8)?,
        confirmation_layers: row.get(9)?,
        discrete_score: row.get(10)?,
        sm_score: row.get(11)?,
        confluence_layers: row.get(12)?,
        radar_confidence: row.get(13)?,
        radar_window_min: row.get(14)?,
        radar_window_max: row.get(15)?,
        radar_suggested_sl: row.get(16)?,
        dip_price: row.get(17)?,
        dip_time: row.get(18)?,
        bars_since_dip: row.get(19)?,
        reversal_detected: opt_bool(row, 20)?,
        reversal_strength: row.get(21)?,
        bounce_from_dip: row.get(22)?,
        bounce_r: row.get(23)?,
        spring_detected: opt_bool(row, 24)?,
        peak_price: row.get(25)?,
        peak_time: row.get(26)?,
        bars_since_peak: row.get(27)?,
        peak_reversal_detected: opt_bool(row, 28)?,
        decline_strength: row.get(29)?,
        decline_from_peak: row.get(30)?,
        decline_r: row.get(31)?,
        upthrust_detected: opt_bool(row, 32)?,
        mtf_support_near: opt_bool(row, 33)?,
        ltf_structure_ok: opt_bool(row, 34)?,
        fib_elliott_zone: opt_bool(row, 35)?,
        divergence_ok: opt_bool(row, 36)?,
        confluence_spring_ok: opt_bool(row, 37)?,
        rsi_zone_ok: opt_bool(row, 38)?,
        bos_ok: opt_bool(row, 39)?,
        absorption_ok: opt_bool(row, 40)?,
        rsi_14: row.get(41)?,
        atr_14: row.get(42)?,
        macd_line: row.get(43)?,
        macd_signal: row.get(44)?,
        macd_hist: row.get(45)?,
        bb_lower: row.get(46)?,
        bb_middle: row.get(47)?,
        bb_upper: row.get(48)?,
        ema_20: row.get(49)?,
        ema_50: row.get(50)?,
        ema_200: row.get(51)?,
        vwap_val: row.get(52)?,
        elliott_formation: row.get(53)?,
        elliott_type: row.get(54)?,
        elliott_in_progress: opt_bool(row, 55)?,
        elliott_validation_ok: opt_bool(row, 56)?,
        elliott_w5_t1: row.get(57)?,
        elliott_w5_t2: row.get(58)?,
        elliott_w5_t3: row.get(59)?,
        classic_pattern: row.get(60)?,
        scenario_role: row.get(61)?,
        scenario_direction: row.get(62)?,
        scenario_entry: row.get(63)?,
        scenario_stop: row.get(64)?,
        scenario_tp1: row.get(65)?,
        scenario_tp2: row.get(66)?,
        scenario_tp3: row.get(67)?,
        scenario_qscore: row.get(68)?,
        scenario_has_radar: opt_bool(row, 69)?,
        po3_phase: row.get(70)?,
        position_state: row.get(71)?,
        market_mode: row.get(72)?,
        local_trend: row.get(73)?,
        global_trend: row.get(74)?,
        volatility_pct: row.get(75)?,
        momentum_short: row.get(76)?,
        momentum_long: row.get(77)?,
        rr: row.get(78)?,
        tmr_trend_points: row.get(79)?,
        tmr_momentum_points: row.get(80)?,
        tmr_rr_points: row.get(81)?,
        tmr_strength_points: row.get(82)?,
        trend_exhaustion: opt_bool(row, 83)?,
        structure_shift: opt_bool(row, 84)?,
        position_side: row.get(85)?,
        extra_json: row.get(86)?,
    })
}

/// Sembol bazlı kar/zarar özeti (web API ve raporlar için).
#[derive(Debug, Clone, Serialize)]
pub struct SymbolPnlStats {
    pub symbol: String,
    pub opened_count: u32,
    pub closed_count: u32,
    pub winners: u32,
    pub losers: u32,
    pub win_rate_pct: f64,
    pub total_pnl: f64,
    pub daily_pnl: f64,
    pub weekly_pnl: f64,
    pub monthly_pnl: f64,
    pub yearly_pnl: f64,
}

const DEFAULT_DB_PATH: &str = "data/trades.db";

fn map_q_analiz_row(row: &rusqlite::Row<'_>) -> SqlResult<QAnalizDetectionRecord> {
    Ok(QAnalizDetectionRecord {
        id: row.get(0)?,
        symbol: row.get(1)?,
        timeframe: row.get(2)?,
        detection: row.get(3)?,
        direction: row.get(4)?,
        confidence_score: row.get(5)?,
        early_warning_score: row.get(6)?,
        recommendation: row.get(7)?,
        reference_price: row.get(8)?,
        confirmation_layers: row.get(9)?,
        created_at: row.get(10)?,
    })
}

fn map_analysis_outcome_row(row: &rusqlite::Row<'_>) -> SqlResult<AnalysisOutcomeRecord> {
    Ok(AnalysisOutcomeRecord {
        id: row.get(0)?,
        event_id: row.get(1)?,
        symbol: row.get(2)?,
        timeframe: row.get(3)?,
        direction: row.get(4)?,
        recommendation: row.get(5)?,
        reference_price: row.get(6)?,
        horizon_bars: row.get(7)?,
        return_pct: row.get(8)?,
        mfe_pct: row.get(9)?,
        mae_pct: row.get(10)?,
        tp_hit: opt_bool(row, 11)?,
        sl_hit: opt_bool(row, 12)?,
        quality_label: row.get(13)?,
        mode: row.get(14)?,
        created_at: row.get(15)?,
    })
}

fn map_trade_analysis_link_row(row: &rusqlite::Row<'_>) -> SqlResult<TradeAnalysisLink> {
    Ok(TradeAnalysisLink {
        id: row.get(0)?,
        position_id: row.get(1)?,
        signal_id: row.get(2)?,
        symbol: row.get(3)?,
        timeframe: row.get(4)?,
        q_event_id: row.get(5)?,
        snapshot_symbol: row.get(6)?,
        snapshot_timeframe: row.get(7)?,
        snapshot_updated_at: row.get(8)?,
        mode: row.get(9)?,
        created_at: row.get(10)?,
    })
}

pub struct TradeDb {
    conn: Connection,
}

impl TradeDb {
    pub fn open(path: Option<&str>) -> SqlResult<Self> {
        let db_path = path.unwrap_or(DEFAULT_DB_PATH);
        if let Some(parent) = std::path::Path::new(db_path).parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let conn = Connection::open(db_path)?;
        let db = Self { conn };
        db.init_tables()?;
        Ok(db)
    }

    fn init_tables(&self) -> SqlResult<()> {
        self.conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS signals (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp   INTEGER NOT NULL,
                symbol      TEXT NOT NULL,
                timeframe   TEXT NOT NULL,
                source      TEXT NOT NULL,
                side        TEXT NOT NULL,
                entry       REAL NOT NULL,
                stop_loss   REAL NOT NULL,
                take_profit REAL NOT NULL,
                score       REAL NOT NULL,
                rr          REAL NOT NULL,
                accepted    INTEGER NOT NULL DEFAULT 0,
                reject_reason TEXT,
                mode        TEXT NOT NULL DEFAULT 'dry'
            );

            CREATE TABLE IF NOT EXISTS positions (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                signal_id   INTEGER,
                opened_at   INTEGER NOT NULL,
                symbol      TEXT NOT NULL,
                timeframe   TEXT NOT NULL,
                source      TEXT NOT NULL,
                side        TEXT NOT NULL,
                entry_price REAL NOT NULL,
                quantity    REAL NOT NULL,
                stop_loss   REAL NOT NULL,
                take_profit REAL NOT NULL,
                current_sl  REAL NOT NULL,
                order_id    TEXT,
                status      TEXT NOT NULL DEFAULT 'open',
                closed_at   INTEGER,
                exit_price  REAL,
                pnl         REAL,
                pnl_r       REAL,
                close_reason TEXT,
                mode        TEXT NOT NULL DEFAULT 'dry',
                FOREIGN KEY (signal_id) REFERENCES signals(id)
            );

            CREATE TABLE IF NOT EXISTS trade_log (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp   INTEGER NOT NULL,
                symbol      TEXT NOT NULL,
                source      TEXT NOT NULL,
                side        TEXT NOT NULL,
                entry       REAL NOT NULL,
                exit_price  REAL NOT NULL,
                quantity    REAL NOT NULL,
                pnl         REAL NOT NULL,
                pnl_r       REAL NOT NULL,
                reason      TEXT NOT NULL,
                mode        TEXT NOT NULL DEFAULT 'dry'
            );

            CREATE TABLE IF NOT EXISTS daily_summary (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                date        TEXT NOT NULL,
                total_trades INTEGER NOT NULL,
                winners     INTEGER NOT NULL,
                losers      INTEGER NOT NULL,
                win_rate    REAL NOT NULL,
                total_pnl   REAL NOT NULL,
                avg_r       REAL NOT NULL,
                mode        TEXT NOT NULL DEFAULT 'dry'
            );

            CREATE TABLE IF NOT EXISTS q_analiz_detections (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                symbol      TEXT NOT NULL,
                timeframe   TEXT NOT NULL,
                detection   TEXT NOT NULL,
                direction   TEXT NOT NULL,
                confidence_score REAL NOT NULL,
                early_warning_score REAL NOT NULL,
                recommendation   TEXT NOT NULL,
                reference_price  REAL NOT NULL,
                confirmation_layers TEXT,
                created_at  INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_q_analiz_created ON q_analiz_detections(created_at DESC);
            CREATE INDEX IF NOT EXISTS idx_q_analiz_symbol ON q_analiz_detections(symbol);

            CREATE TABLE IF NOT EXISTS trade_analysis_links (
                id                  INTEGER PRIMARY KEY AUTOINCREMENT,
                position_id         INTEGER NOT NULL,
                signal_id           INTEGER NOT NULL,
                symbol              TEXT NOT NULL,
                timeframe           TEXT NOT NULL,
                q_event_id          INTEGER,
                snapshot_symbol     TEXT,
                snapshot_timeframe  TEXT,
                snapshot_updated_at INTEGER,
                mode                TEXT NOT NULL DEFAULT 'dry',
                created_at          INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_trade_link_position ON trade_analysis_links(position_id);
            CREATE INDEX IF NOT EXISTS idx_trade_link_signal ON trade_analysis_links(signal_id);

            CREATE TABLE IF NOT EXISTS analysis_outcomes (
                id              INTEGER PRIMARY KEY AUTOINCREMENT,
                event_id        INTEGER NOT NULL,
                symbol          TEXT NOT NULL,
                timeframe       TEXT NOT NULL,
                direction       TEXT NOT NULL,
                recommendation  TEXT NOT NULL,
                reference_price REAL NOT NULL,
                horizon_bars    INTEGER NOT NULL,
                return_pct      REAL NOT NULL,
                mfe_pct         REAL,
                mae_pct         REAL,
                tp_hit          INTEGER,
                sl_hit          INTEGER,
                quality_label   TEXT,
                mode            TEXT NOT NULL DEFAULT 'dry',
                created_at      INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_outcomes_event_horizon ON analysis_outcomes(event_id, horizon_bars);
            CREATE INDEX IF NOT EXISTS idx_outcomes_symbol_tf_created ON analysis_outcomes(symbol, timeframe, created_at DESC);

            CREATE TABLE IF NOT EXISTS analysis_snapshots (
                symbol                  TEXT NOT NULL,
                timeframe               TEXT NOT NULL,
                updated_at              INTEGER NOT NULL,
                detection               TEXT NOT NULL DEFAULT '—',
                direction               TEXT NOT NULL DEFAULT '—',
                recommendation          TEXT NOT NULL DEFAULT '—',
                confidence_score        REAL NOT NULL DEFAULT 0,
                early_warning_score     REAL NOT NULL DEFAULT 0,
                reference_price         REAL NOT NULL DEFAULT 0,
                confirmation_layers     TEXT,
                discrete_score         REAL,
                sm_score                REAL,
                confluence_layers      INTEGER,
                radar_confidence       REAL,
                radar_window_min        INTEGER,
                radar_window_max        INTEGER,
                radar_suggested_sl     REAL,
                dip_price               REAL,
                dip_time                INTEGER,
                bars_since_dip         INTEGER,
                reversal_detected       INTEGER,
                reversal_strength       REAL,
                bounce_from_dip         REAL,
                bounce_r                REAL,
                spring_detected         INTEGER,
                peak_price              REAL,
                peak_time               INTEGER,
                bars_since_peak         INTEGER,
                peak_reversal_detected  INTEGER,
                decline_strength        REAL,
                decline_from_peak       REAL,
                decline_r               REAL,
                upthrust_detected       INTEGER,
                mtf_support_near        INTEGER,
                ltf_structure_ok        INTEGER,
                fib_elliott_zone        INTEGER,
                divergence_ok           INTEGER,
                confluence_spring_ok    INTEGER,
                rsi_zone_ok             INTEGER,
                bos_ok                  INTEGER,
                absorption_ok           INTEGER,
                rsi_14                  REAL,
                atr_14                  REAL,
                macd_line               REAL,
                macd_signal             REAL,
                macd_hist               REAL,
                bb_lower                REAL,
                bb_middle               REAL,
                bb_upper                REAL,
                ema_20                  REAL,
                ema_50                  REAL,
                ema_200                 REAL,
                vwap_val                REAL,
                elliott_formation       TEXT,
                elliott_type            TEXT,
                elliott_in_progress     INTEGER,
                elliott_validation_ok   INTEGER,
                elliott_w5_t1           REAL,
                elliott_w5_t2           REAL,
                elliott_w5_t3           REAL,
                classic_pattern         TEXT,
                scenario_role           TEXT,
                scenario_direction      TEXT,
                scenario_entry          REAL,
                scenario_stop           REAL,
                scenario_tp1            REAL,
                scenario_tp2            REAL,
                scenario_tp3            REAL,
                scenario_qscore         REAL,
                scenario_has_radar      INTEGER,
                po3_phase               TEXT,
                position_state          TEXT,
                market_mode             TEXT,
                local_trend             INTEGER,
                global_trend            INTEGER,
                volatility_pct          REAL,
                momentum_short          REAL,
                momentum_long           REAL,
                rr                      REAL,
                tmr_trend_points        INTEGER,
                tmr_momentum_points     INTEGER,
                tmr_rr_points           INTEGER,
                tmr_strength_points     INTEGER,
                trend_exhaustion        INTEGER,
                structure_shift         INTEGER,
                position_side           TEXT,
                extra_json              TEXT,
                PRIMARY KEY (symbol, timeframe)
            );
            DROP TABLE IF EXISTS metrics_snapshots;
            "
        )?;
        // Migration: analysis_snapshots'a pozisyon metrik kolonları (eski DB'ler için)
        let alter_cols = [
            "position_state TEXT",
            "market_mode TEXT",
            "local_trend INTEGER",
            "global_trend INTEGER",
            "volatility_pct REAL",
            "momentum_short REAL",
            "momentum_long REAL",
            "rr REAL",
            "tmr_trend_points INTEGER",
            "tmr_momentum_points INTEGER",
            "tmr_rr_points INTEGER",
            "tmr_strength_points INTEGER",
            "trend_exhaustion INTEGER",
            "structure_shift INTEGER",
            "position_side TEXT",
        ];
        for col in alter_cols {
            let sql = format!("ALTER TABLE analysis_snapshots ADD COLUMN {}", col);
            let _ = self.conn.execute(&sql, ());
        }
        Ok(())
    }

    /// Gelen sinyali kaydet, dönen id signal_id olarak kullanılır.
    pub fn insert_signal(
        &self,
        signal: &TradeSignal,
        accepted: bool,
        reject_reason: Option<&str>,
        mode: TradingMode,
    ) -> SqlResult<i64> {
        self.conn.execute(
            "INSERT INTO signals (timestamp, symbol, timeframe, source, side, entry, stop_loss, take_profit, score, rr, accepted, reject_reason, mode)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                signal.timestamp,
                signal.symbol,
                signal.timeframe.to_binance_interval(),
                signal.source.to_string(),
                if signal.is_long { "LONG" } else { "SHORT" },
                signal.entry,
                signal.stop_loss,
                signal.take_profit,
                signal.score,
                signal.rr,
                accepted as i32,
                reject_reason,
                mode.as_str(),
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Açık pozisyon kaydet.
    pub fn insert_position(
        &self,
        signal_id: i64,
        managed: &ManagedPosition,
        mode: TradingMode,
    ) -> SqlResult<i64> {
        self.conn.execute(
            "INSERT INTO positions (signal_id, opened_at, symbol, timeframe, source, side, entry_price, quantity, stop_loss, take_profit, current_sl, order_id, status, mode)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, 'open', ?13)",
            params![
                signal_id,
                managed.opened_at,
                managed.signal.symbol,
                managed.signal.timeframe.to_binance_interval(),
                managed.signal.source.to_string(),
                if managed.signal.is_long { "LONG" } else { "SHORT" },
                managed.position.entry_price,
                managed.position.quantity,
                managed.position.initial_sl,
                managed.position.initial_tp,
                managed.position.current_sl,
                managed.order_id,
                mode.as_str(),
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Pozisyonu kapat (status=closed, exit bilgileri yaz).
    pub fn close_position(
        &self,
        position_db_id: i64,
        exit_price: f64,
        pnl: f64,
        pnl_r: f64,
        reason: &str,
    ) -> SqlResult<()> {
        let now = chrono::Utc::now().timestamp_millis();
        self.conn.execute(
            "UPDATE positions SET status='closed', closed_at=?1, exit_price=?2, pnl=?3, pnl_r=?4, close_reason=?5 WHERE id=?6",
            params![now, exit_price, pnl, pnl_r, reason, position_db_id],
        )?;
        Ok(())
    }

    /// Pozisyonun current_sl güncellemesi (trailing stop, breakeven).
    pub fn update_position_sl(&self, position_db_id: i64, new_sl: f64) -> SqlResult<()> {
        self.conn.execute(
            "UPDATE positions SET current_sl=?1 WHERE id=?2",
            params![new_sl, position_db_id],
        )?;
        Ok(())
    }

    /// Trade log kaydı.
    pub fn insert_trade_log(&self, log: &TradeLog, mode: TradingMode) -> SqlResult<()> {
        self.conn.execute(
            "INSERT INTO trade_log (timestamp, symbol, source, side, entry, exit_price, quantity, pnl, pnl_r, reason, mode)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                log.timestamp,
                log.symbol,
                log.source.to_string(),
                log.side,
                log.entry,
                log.exit,
                log.quantity,
                log.pnl,
                log.pnl_r,
                log.reason,
                mode.as_str(),
            ],
        )?;
        Ok(())
    }

    /// Günlük özet kaydet.
    pub fn insert_daily_summary(
        &self,
        date: &str,
        total_trades: usize,
        winners: usize,
        losers: usize,
        win_rate: f64,
        total_pnl: f64,
        avg_r: f64,
        mode: TradingMode,
    ) -> SqlResult<()> {
        self.conn.execute(
            "INSERT INTO daily_summary (date, total_trades, winners, losers, win_rate, total_pnl, avg_r, mode)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![date, total_trades as i32, winners as i32, losers as i32, win_rate, total_pnl, avg_r, mode.as_str()],
        )?;
        Ok(())
    }

    /// Bugünkü açık pozisyonları getir (restart sonrası recovery).
    /// Dönen tuple: (id, opened_at, symbol, timeframe, source, side, entry_price, quantity, stop_loss, take_profit, current_sl, order_id).
    pub fn load_open_positions(&self, mode: TradingMode) -> SqlResult<Vec<(i64, i64, String, String, String, String, f64, f64, f64, f64, f64, String)>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, opened_at, symbol, timeframe, source, side, entry_price, quantity, stop_loss, take_profit, current_sl, order_id
             FROM positions WHERE status='open' AND mode=?1"
        )?;
        let rows = stmt.query_map(params![mode.as_str()], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
                row.get(6)?,
                row.get(7)?,
                row.get(8)?,
                row.get(9)?,
                row.get(10)?,
                row.get::<_, String>(11)?,
            ))
        })?;
        rows.collect()
    }

    /// Analiz snapshot upsert: her (symbol, timeframe) için tek satır, her turda güncellenir.
    pub fn upsert_analysis_snapshot(&self, s: &AnalysisSnapshot) -> SqlResult<()> {
        let now = chrono::Utc::now().timestamp_millis();
        let tf = s.timeframe.to_binance_interval();
        let b = |v: Option<bool>| v.map(|x| x as i32);
        self.conn.execute(
            "INSERT INTO analysis_snapshots (
                symbol, timeframe, updated_at, detection, direction, recommendation,
                confidence_score, early_warning_score, reference_price, confirmation_layers,
                discrete_score, sm_score, confluence_layers,
                radar_confidence, radar_window_min, radar_window_max, radar_suggested_sl,
                dip_price, dip_time, bars_since_dip, reversal_detected, reversal_strength, bounce_from_dip, bounce_r, spring_detected,
                peak_price, peak_time, bars_since_peak, peak_reversal_detected, decline_strength, decline_from_peak, decline_r, upthrust_detected,
                mtf_support_near, ltf_structure_ok, fib_elliott_zone, divergence_ok, confluence_spring_ok, rsi_zone_ok, bos_ok, absorption_ok,
                rsi_14, atr_14, macd_line, macd_signal, macd_hist, bb_lower, bb_middle, bb_upper, ema_20, ema_50, ema_200, vwap_val,
                elliott_formation, elliott_type, elliott_in_progress, elliott_validation_ok, elliott_w5_t1, elliott_w5_t2, elliott_w5_t3,
                classic_pattern, scenario_role, scenario_direction, scenario_entry, scenario_stop, scenario_tp1, scenario_tp2, scenario_tp3, scenario_qscore, scenario_has_radar,
                po3_phase, position_state, market_mode, local_trend, global_trend, volatility_pct, momentum_short, momentum_long, rr, tmr_trend_points, tmr_momentum_points, tmr_rr_points, tmr_strength_points, trend_exhaustion, structure_shift, position_side, extra_json
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17,
                ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28, ?29, ?30, ?31, ?32, ?33,
                ?34, ?35, ?36, ?37, ?38, ?39, ?40, ?41, ?42, ?43, ?44, ?45, ?46, ?47, ?48, ?49, ?50, ?51, ?52, ?53, ?54,
                ?55, ?56, ?57, ?58, ?59, ?60, ?61, ?62, ?63, ?64, ?65, ?66, ?67, ?68, ?69, ?70, ?71, ?72,
                ?73, ?74, ?75, ?76, ?77, ?78, ?79, ?80, ?81, ?82, ?83, ?84, ?85, ?86, ?87
            )
            ON CONFLICT(symbol, timeframe) DO UPDATE SET
                updated_at=excluded.updated_at, detection=excluded.detection, direction=excluded.direction, recommendation=excluded.recommendation,
                confidence_score=excluded.confidence_score, early_warning_score=excluded.early_warning_score, reference_price=excluded.reference_price, confirmation_layers=excluded.confirmation_layers,
                discrete_score=excluded.discrete_score, sm_score=excluded.sm_score, confluence_layers=excluded.confluence_layers,
                radar_confidence=excluded.radar_confidence, radar_window_min=excluded.radar_window_min, radar_window_max=excluded.radar_window_max, radar_suggested_sl=excluded.radar_suggested_sl,
                dip_price=excluded.dip_price, dip_time=excluded.dip_time, bars_since_dip=excluded.bars_since_dip, reversal_detected=excluded.reversal_detected, reversal_strength=excluded.reversal_strength, bounce_from_dip=excluded.bounce_from_dip, bounce_r=excluded.bounce_r, spring_detected=excluded.spring_detected,
                peak_price=excluded.peak_price, peak_time=excluded.peak_time, bars_since_peak=excluded.bars_since_peak, peak_reversal_detected=excluded.peak_reversal_detected, decline_strength=excluded.decline_strength, decline_from_peak=excluded.decline_from_peak, decline_r=excluded.decline_r, upthrust_detected=excluded.upthrust_detected,
                mtf_support_near=excluded.mtf_support_near, ltf_structure_ok=excluded.ltf_structure_ok, fib_elliott_zone=excluded.fib_elliott_zone, divergence_ok=excluded.divergence_ok, confluence_spring_ok=excluded.confluence_spring_ok, rsi_zone_ok=excluded.rsi_zone_ok, bos_ok=excluded.bos_ok, absorption_ok=excluded.absorption_ok,
                rsi_14=excluded.rsi_14, atr_14=excluded.atr_14, macd_line=excluded.macd_line, macd_signal=excluded.macd_signal, macd_hist=excluded.macd_hist, bb_lower=excluded.bb_lower, bb_middle=excluded.bb_middle, bb_upper=excluded.bb_upper, ema_20=excluded.ema_20, ema_50=excluded.ema_50, ema_200=excluded.ema_200, vwap_val=excluded.vwap_val,
                elliott_formation=excluded.elliott_formation, elliott_type=excluded.elliott_type, elliott_in_progress=excluded.elliott_in_progress, elliott_validation_ok=excluded.elliott_validation_ok, elliott_w5_t1=excluded.elliott_w5_t1, elliott_w5_t2=excluded.elliott_w5_t2, elliott_w5_t3=excluded.elliott_w5_t3,
                classic_pattern=excluded.classic_pattern, scenario_role=excluded.scenario_role, scenario_direction=excluded.scenario_direction, scenario_entry=excluded.scenario_entry, scenario_stop=excluded.scenario_stop, scenario_tp1=excluded.scenario_tp1, scenario_tp2=excluded.scenario_tp2, scenario_tp3=excluded.scenario_tp3, scenario_qscore=excluded.scenario_qscore, scenario_has_radar=excluded.scenario_has_radar,
                po3_phase=excluded.po3_phase, position_state=excluded.position_state, market_mode=excluded.market_mode, local_trend=excluded.local_trend, global_trend=excluded.global_trend, volatility_pct=excluded.volatility_pct, momentum_short=excluded.momentum_short, momentum_long=excluded.momentum_long, rr=excluded.rr, tmr_trend_points=excluded.tmr_trend_points, tmr_momentum_points=excluded.tmr_momentum_points, tmr_rr_points=excluded.tmr_rr_points, tmr_strength_points=excluded.tmr_strength_points, trend_exhaustion=excluded.trend_exhaustion, structure_shift=excluded.structure_shift, position_side=excluded.position_side, extra_json=excluded.extra_json",
            params![
                s.symbol, tf, now, s.detection, s.direction, s.recommendation,
                s.confidence_score, s.early_warning_score, s.reference_price, s.confirmation_layers,
                s.discrete_score, s.sm_score, s.confluence_layers,
                s.radar_confidence, s.radar_window_min, s.radar_window_max, s.radar_suggested_sl,
                s.dip_price, s.dip_time, s.bars_since_dip, b(s.reversal_detected), s.reversal_strength, s.bounce_from_dip, s.bounce_r, b(s.spring_detected),
                s.peak_price, s.peak_time, s.bars_since_peak, b(s.peak_reversal_detected), s.decline_strength, s.decline_from_peak, s.decline_r, b(s.upthrust_detected),
                b(s.mtf_support_near), b(s.ltf_structure_ok), b(s.fib_elliott_zone), b(s.divergence_ok), b(s.confluence_spring_ok), b(s.rsi_zone_ok), b(s.bos_ok), b(s.absorption_ok),
                s.rsi_14, s.atr_14, s.macd_line, s.macd_signal, s.macd_hist, s.bb_lower, s.bb_middle, s.bb_upper, s.ema_20, s.ema_50, s.ema_200, s.vwap_val,
                s.elliott_formation, s.elliott_type, s.elliott_in_progress, b(s.elliott_validation_ok), s.elliott_w5_t1, s.elliott_w5_t2, s.elliott_w5_t3,
                s.classic_pattern, s.scenario_role, s.scenario_direction, s.scenario_entry, s.scenario_stop, s.scenario_tp1, s.scenario_tp2, s.scenario_tp3, s.scenario_qscore, b(s.scenario_has_radar),
                s.po3_phase, s.position_state.clone(), s.market_mode.clone(), s.local_trend, s.global_trend, s.volatility_pct, s.momentum_short, s.momentum_long, s.rr, s.tmr_trend_points, s.tmr_momentum_points, s.tmr_rr_points, s.tmr_strength_points, b(s.trend_exhaustion), b(s.structure_shift), s.position_side.clone(), s.extra_json,
            ],
        )?;
        Ok(())
    }

    /// Analiz snapshot'larını listele (sembol filtresi opsiyonel). Daemon'un yazdığı tablodan okur.
    pub fn get_analysis_snapshots(&self, symbol_filter: Option<&str>) -> SqlResult<Vec<AnalysisSnapshotRecord>> {
        let cols = "symbol, timeframe, updated_at, detection, direction, recommendation,
            confidence_score, early_warning_score, reference_price, confirmation_layers,
            discrete_score, sm_score, confluence_layers,
            radar_confidence, radar_window_min, radar_window_max, radar_suggested_sl,
            dip_price, dip_time, bars_since_dip, reversal_detected, reversal_strength, bounce_from_dip, bounce_r, spring_detected,
            peak_price, peak_time, bars_since_peak, peak_reversal_detected, decline_strength, decline_from_peak, decline_r, upthrust_detected,
            mtf_support_near, ltf_structure_ok, fib_elliott_zone, divergence_ok, confluence_spring_ok, rsi_zone_ok, bos_ok, absorption_ok,
            rsi_14, atr_14, macd_line, macd_signal, macd_hist, bb_lower, bb_middle, bb_upper, ema_20, ema_50, ema_200, vwap_val,
            elliott_formation, elliott_type, elliott_in_progress, elliott_validation_ok, elliott_w5_t1, elliott_w5_t2, elliott_w5_t3,
            classic_pattern, scenario_role, scenario_direction, scenario_entry, scenario_stop, scenario_tp1, scenario_tp2, scenario_tp3, scenario_qscore, scenario_has_radar,
            po3_phase, position_state, market_mode, local_trend, global_trend, volatility_pct, momentum_short, momentum_long, rr, tmr_trend_points, tmr_momentum_points, tmr_rr_points, tmr_strength_points, trend_exhaustion, structure_shift, position_side, extra_json";
        if let Some(sym) = symbol_filter {
            let sql = format!("SELECT {} FROM analysis_snapshots WHERE symbol = ?1 ORDER BY symbol, timeframe", cols);
            let mut stmt = self.conn.prepare(&sql)?;
            stmt.query_map(params![sym], map_analysis_snapshot_row).and_then(|r| r.collect::<SqlResult<Vec<AnalysisSnapshotRecord>>>())
        } else {
            let sql = format!("SELECT {} FROM analysis_snapshots ORDER BY symbol, timeframe", cols);
            let mut stmt = self.conn.prepare(&sql)?;
            stmt.query_map((), map_analysis_snapshot_row).and_then(|r| r.collect::<SqlResult<Vec<AnalysisSnapshotRecord>>>())
        }
    }

    /// Q-Analiz tespitini kaydet (daemon taramalarından).
    pub fn insert_q_analiz_detection(&self, opp: &QRadarOpportunityAnalysis) -> SqlResult<i64> {
        let now = chrono::Utc::now().timestamp_millis();
        self.conn.execute(
            "INSERT INTO q_analiz_detections (symbol, timeframe, detection, direction, confidence_score, early_warning_score, recommendation, reference_price, confirmation_layers, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                opp.symbol,
                opp.timeframe.to_binance_interval(),
                opp.detection,
                opp.direction,
                opp.confidence_score,
                opp.early_warning_score,
                opp.recommendation,
                opp.reference_price,
                opp.confirmation_layers,
                now,
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Q-Analiz tespit kayıtlarını listele (yeniden eskiye, isteğe bağlı sembol filtresi).
    pub fn get_q_analiz_detections(
        &self,
        limit: u32,
        symbol_filter: Option<&str>,
    ) -> SqlResult<Vec<QAnalizDetectionRecord>> {
        let limit = limit.min(500) as i32;
        let mut stmt = if symbol_filter.is_some() {
            self.conn.prepare(
                "SELECT id, symbol, timeframe, detection, direction, confidence_score, early_warning_score, recommendation, reference_price, confirmation_layers, created_at
                 FROM q_analiz_detections WHERE symbol = ?1 ORDER BY created_at DESC LIMIT ?2",
            )?
        } else {
            self.conn.prepare(
                "SELECT id, symbol, timeframe, detection, direction, confidence_score, early_warning_score, recommendation, reference_price, confirmation_layers, created_at
                 FROM q_analiz_detections ORDER BY created_at DESC LIMIT ?1",
            )?
        };
        let rows = if let Some(sym) = symbol_filter {
            stmt.query_map(params![sym, limit], map_q_analiz_row)?
        } else {
            stmt.query_map(params![limit], map_q_analiz_row)?
        };
        rows.collect()
    }

    /// Verilen sembol/timeframe ve zaman için en yakın (geriye dönük) Q-Analiz event'ini döndürür.
    /// `window_ms` içinde değilse None döner.
    pub fn find_recent_q_event_for(
        &self,
        symbol: &str,
        timeframe: &str,
        ts_ms: i64,
        window_ms: i64,
    ) -> SqlResult<Option<QAnalizDetectionRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, symbol, timeframe, detection, direction, confidence_score, early_warning_score,
                    recommendation, reference_price, confirmation_layers, created_at
             FROM q_analiz_detections
             WHERE symbol = ?1 AND timeframe = ?2 AND created_at <= ?3
             ORDER BY created_at DESC
             LIMIT 1",
        )?;
        let mut rows = stmt.query(params![symbol, timeframe, ts_ms])?;
        if let Some(row) = rows.next()? {
            let rec = map_q_analiz_row(&row)?;
            if ts_ms - rec.created_at <= window_ms {
                Ok(Some(rec))
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }

    /// Event sonrası outcome kaydı ekler (her event + horizon için bir satır).
    pub fn insert_analysis_outcome(
        &self,
        event_id: i64,
        symbol: &str,
        timeframe: &str,
        direction: &str,
        recommendation: &str,
        reference_price: f64,
        horizon_bars: i32,
        return_pct: f64,
        mfe_pct: Option<f64>,
        mae_pct: Option<f64>,
        tp_hit: Option<bool>,
        sl_hit: Option<bool>,
        quality_label: Option<&str>,
        mode: &str,
    ) -> SqlResult<i64> {
        let now = chrono::Utc::now().timestamp_millis();
        self.conn.execute(
            "INSERT INTO analysis_outcomes (
                event_id, symbol, timeframe, direction, recommendation, reference_price,
                horizon_bars, return_pct, mfe_pct, mae_pct, tp_hit, sl_hit, quality_label, mode, created_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
            params![
                event_id,
                symbol,
                timeframe,
                direction,
                recommendation,
                reference_price,
                horizon_bars,
                return_pct,
                mfe_pct,
                mae_pct,
                tp_hit.map(|v| if v { 1 } else { 0 }),
                sl_hit.map(|v| if v { 1 } else { 0 }),
                quality_label,
                mode,
                now
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Outcome kayıtlarını listeler (event/symbol filtreli, yeniden eskiye).
    pub fn get_analysis_outcomes(
        &self,
        limit: u32,
        symbol_filter: Option<&str>,
        event_id_filter: Option<i64>,
    ) -> SqlResult<Vec<AnalysisOutcomeRecord>> {
        let limit = limit.min(1000) as i32;
        match (symbol_filter, event_id_filter) {
            (Some(sym), Some(event_id)) => {
                let mut stmt = self.conn.prepare(
                    "SELECT id, event_id, symbol, timeframe, direction, recommendation, reference_price,
                            horizon_bars, return_pct, mfe_pct, mae_pct, tp_hit, sl_hit, quality_label, mode, created_at
                     FROM analysis_outcomes
                     WHERE symbol = ?1 AND event_id = ?2
                     ORDER BY created_at DESC LIMIT ?3",
                )?;
                let rows = stmt.query_map(params![sym, event_id, limit], map_analysis_outcome_row)?;
                rows.collect()
            }
            (Some(sym), None) => {
                let mut stmt = self.conn.prepare(
                    "SELECT id, event_id, symbol, timeframe, direction, recommendation, reference_price,
                            horizon_bars, return_pct, mfe_pct, mae_pct, tp_hit, sl_hit, quality_label, mode, created_at
                     FROM analysis_outcomes
                     WHERE symbol = ?1
                     ORDER BY created_at DESC LIMIT ?2",
                )?;
                let rows = stmt.query_map(params![sym, limit], map_analysis_outcome_row)?;
                rows.collect()
            }
            (None, Some(event_id)) => {
                let mut stmt = self.conn.prepare(
                    "SELECT id, event_id, symbol, timeframe, direction, recommendation, reference_price,
                            horizon_bars, return_pct, mfe_pct, mae_pct, tp_hit, sl_hit, quality_label, mode, created_at
                     FROM analysis_outcomes
                     WHERE event_id = ?1
                     ORDER BY created_at DESC LIMIT ?2",
                )?;
                let rows = stmt.query_map(params![event_id, limit], map_analysis_outcome_row)?;
                rows.collect()
            }
            (None, None) => {
                let mut stmt = self.conn.prepare(
                    "SELECT id, event_id, symbol, timeframe, direction, recommendation, reference_price,
                            horizon_bars, return_pct, mfe_pct, mae_pct, tp_hit, sl_hit, quality_label, mode, created_at
                     FROM analysis_outcomes
                     ORDER BY created_at DESC LIMIT ?1",
                )?;
                let rows = stmt.query_map(params![limit], map_analysis_outcome_row)?;
                rows.collect()
            }
        }
    }

    /// Trade ile analiz state/event'ini bağlayan link kaydı ekler.
    pub fn insert_trade_analysis_link(
        &self,
        position_id: i64,
        signal_id: i64,
        symbol: &str,
        timeframe: &str,
        q_event_id: Option<i64>,
        snapshot_symbol: Option<&str>,
        snapshot_timeframe: Option<&str>,
        snapshot_updated_at: Option<i64>,
        mode: &str,
    ) -> SqlResult<i64> {
        let now = chrono::Utc::now().timestamp_millis();
        self.conn.execute(
            "INSERT INTO trade_analysis_links (
                position_id, signal_id, symbol, timeframe,
                q_event_id, snapshot_symbol, snapshot_timeframe, snapshot_updated_at, mode, created_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                position_id,
                signal_id,
                symbol,
                timeframe,
                q_event_id,
                snapshot_symbol,
                snapshot_timeframe,
                snapshot_updated_at,
                mode,
                now
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Verilen pozisyon için link kayıtlarını döndürür (yeniden eskiye).
    pub fn get_trade_analysis_links_by_position(
        &self,
        position_id: i64,
    ) -> SqlResult<Vec<TradeAnalysisLink>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, position_id, signal_id, symbol, timeframe,
                    q_event_id, snapshot_symbol, snapshot_timeframe, snapshot_updated_at, mode, created_at
             FROM trade_analysis_links
             WHERE position_id = ?1
             ORDER BY created_at DESC",
        )?;
        let rows = stmt.query_map(params![position_id], map_trade_analysis_link_row)?;
        rows.collect()
    }

    /// Sembol bazlı PnL özeti: açılan/kapanan sayı, başarı oranı, günlük/haftalık/aylık/yıllık ve toplam kar zarar.
    pub fn get_symbol_pnl_stats(&self, mode: TradingMode) -> SqlResult<Vec<SymbolPnlStats>> {
        let mode_str = mode.as_str();
        let now = chrono::Utc::now();
        let n = now.date_naive();
        let start_day = n.and_hms_opt(0, 0, 0).unwrap().and_utc().timestamp_millis();
        let start_week = n
            .week(chrono::Weekday::Mon)
            .first_day()
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc()
            .timestamp_millis();
        let start_month = n
            .with_day(1)
            .unwrap()
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc()
            .timestamp_millis();
        let start_year = n
            .with_month(1)
            .unwrap()
            .with_day(1)
            .unwrap()
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc()
            .timestamp_millis();

        let mut opened: std::collections::HashMap<String, u32> = std::collections::HashMap::new();
        let mut stmt = self.conn.prepare(
            "SELECT symbol, COUNT(*) FROM positions WHERE mode=?1 GROUP BY symbol",
        )?;
        for row in stmt.query_map(params![mode_str], |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? as u32)))? {
            let (sym, cnt) = row?;
            opened.insert(sym, cnt);
        }

        let mut closed_list: Vec<(String, i64, f64)> = Vec::new();
        let mut stmt = self.conn.prepare(
            "SELECT symbol, closed_at, pnl FROM positions WHERE status='closed' AND mode=?1 AND closed_at IS NOT NULL AND pnl IS NOT NULL",
        )?;
        for row in stmt.query_map(params![mode_str], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?, row.get::<_, f64>(2)?))
        })? {
            closed_list.push(row?);
        }

        let mut by_symbol: std::collections::HashMap<String, (u32, u32, f64, f64, f64, f64, f64)> =
            std::collections::HashMap::new();
        for (symbol, closed_at, pnl) in closed_list {
            let entry = by_symbol.entry(symbol).or_insert((0, 0, 0.0, 0.0, 0.0, 0.0, 0.0));
            entry.0 += 1;
            if pnl > 0.0 {
                entry.1 += 1;
            }
            entry.2 += pnl;
            if closed_at >= start_day {
                entry.3 += pnl;
            }
            if closed_at >= start_week {
                entry.4 += pnl;
            }
            if closed_at >= start_month {
                entry.5 += pnl;
            }
            if closed_at >= start_year {
                entry.6 += pnl;
            }
        }

        let symbols: std::collections::HashSet<String> =
            opened.keys().cloned().chain(by_symbol.keys().cloned()).collect();
        let mut symbols: Vec<String> = symbols.into_iter().collect();
        symbols.sort_unstable();
        let mut out = Vec::with_capacity(symbols.len());
        for symbol in symbols {
            let op = opened.get(&symbol).copied().unwrap_or(0);
            let (closed_count, winners, total_pnl, daily_pnl, weekly_pnl, monthly_pnl, yearly_pnl) =
                by_symbol.get(&symbol).copied().unwrap_or((0, 0, 0.0, 0.0, 0.0, 0.0, 0.0));
            let losers = closed_count.saturating_sub(winners);
            let win_rate_pct = if closed_count > 0 {
                winners as f64 / closed_count as f64 * 100.0
            } else {
                0.0
            };
            out.push(SymbolPnlStats {
                symbol: symbol.clone(),
                opened_count: op,
                closed_count: closed_count,
                winners,
                losers,
                win_rate_pct,
                total_pnl,
                daily_pnl,
                weekly_pnl,
                monthly_pnl,
                yearly_pnl,
            });
        }
        out.sort_by(|a, b| a.symbol.cmp(&b.symbol));
        Ok(out)
    }
}
