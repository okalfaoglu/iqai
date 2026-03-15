//! SQLite tabanlı trade veritabanı.
//! live ve dry modlarda tüm sinyaller, açık/kapalı pozisyonlar ve PnL kaydedilir.

use chrono::Datelike;
use rusqlite::{params, Connection, Result as SqlResult};
use serde::Serialize;

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
            "
        )
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
