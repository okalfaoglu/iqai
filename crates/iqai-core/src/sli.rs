//! TFAI-O06 — SLI başlangıç seti: Prometheus metin formatı + `trade_db` toplu göstergeler.
//!
//! Canlı emir sayaçları `TradeDb::sli_incr` ile `auto_trader` tarafından yazılır.
//! Scrape: `GET /metrics/prometheus` (iqai-web; HTML panel `/metrics` ile çakışmaz).

use std::fmt::Write;

use crate::trade_db::TradeDb;

/// Prometheus exposition formatı (0.0.4). `iqai-web` `/metrics/prometheus` bunu döner.
pub fn render_prometheus_sli(db: &TradeDb) -> rusqlite::Result<String> {
    db.persist_q04_normalized_errors_from_memory()?;
    let snap = db.sli_counters_snapshot()?;

    let mut w = String::new();

    writeln!(
        w,
        "# HELP iqai_info IQAI iqai-core sürümü (SLI exporter)."
    )
    .unwrap();
    writeln!(w, "# TYPE iqai_info gauge").unwrap();
    writeln!(
        w,
        "iqai_info{{version=\"{}\"}} 1",
        env!("CARGO_PKG_VERSION")
    )
    .unwrap();

    writeln!(
        w,
        "# HELP iqai_db_reachable Metrikler için trade DB açılabildi (1=evet)."
    )
    .unwrap();
    writeln!(w, "# TYPE iqai_db_reachable gauge").unwrap();
    writeln!(w, "iqai_db_reachable 1").unwrap();

    writeln!(
        w,
        "# HELP iqai_open_positions Açık pozisyon sayısı (mod bazlı; operasyonel görünürlük)."
    )
    .unwrap();
    writeln!(w, "# TYPE iqai_open_positions gauge").unwrap();
    for (mode, n) in db.count_open_positions_by_mode()? {
        let m = prom_escape_label(&mode);
        writeln!(w, "iqai_open_positions{{mode=\"{m}\"}} {n}").unwrap();
    }

    let now_ms = chrono::Utc::now().timestamp_millis();
    let (min_u, max_u) = db.analysis_snapshots_updated_at_bounds()?;
    if min_u.is_some() || max_u.is_some() {
        writeln!(
            w,
            "# HELP iqai_analysis_snapshot_oldest_age_seconds En eski analysis_snapshots satırının yaşı (veri tazeliği SLI)."
        )
        .unwrap();
        writeln!(w, "# TYPE iqai_analysis_snapshot_oldest_age_seconds gauge").unwrap();
        if let Some(ts) = min_u {
            let sec = ((now_ms.saturating_sub(ts)) / 1000).max(0);
            writeln!(w, "iqai_analysis_snapshot_oldest_age_seconds {sec}").unwrap();
        } else {
            writeln!(w, "iqai_analysis_snapshot_oldest_age_seconds 0").unwrap();
        }

        writeln!(
            w,
            "# HELP iqai_analysis_snapshot_newest_age_seconds En yeni snapshot satırının yaşı (saniye)."
        )
        .unwrap();
        writeln!(w, "# TYPE iqai_analysis_snapshot_newest_age_seconds gauge").unwrap();
        if let Some(ts) = max_u {
            let sec = ((now_ms.saturating_sub(ts)) / 1000).max(0);
            writeln!(w, "iqai_analysis_snapshot_newest_age_seconds {sec}").unwrap();
        } else {
            writeln!(w, "iqai_analysis_snapshot_newest_age_seconds 0").unwrap();
        }
    }

    for (k, v) in &snap {
        if k.starts_with(crate::binance_error::Q04_SLI_KEY_PREFIX) {
            continue;
        }
        // Anahtar zaten Prometheus uyumlu isim (örn. exec_order_open_success_total)
        writeln!(w, "# HELP {k} Canlı emir / yürütme sayacı (iqai-cli auto_trader).").unwrap();
        writeln!(w, "# TYPE {k} counter").unwrap();
        writeln!(w, "{k} {v}").unwrap();
    }

    let q04 = crate::binance_error::prometheus_q04_from_sli_snapshot(&snap);
    if !q04.is_empty() {
        w.push_str(&q04);
    }

    Ok(w)
}

fn prom_escape_label(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// DB açılamadığında veya okuma hatasında yalnızca `iqai_info` + `iqai_db_reachable` döner.
pub fn render_prometheus_sli_minimal(db_reachable: bool) -> String {
    let mut w = String::new();
    writeln!(
        w,
        "# HELP iqai_info IQAI iqai-core sürümü (SLI exporter)."
    )
    .unwrap();
    writeln!(w, "# TYPE iqai_info gauge").unwrap();
    writeln!(
        w,
        "iqai_info{{version=\"{}\"}} 1",
        env!("CARGO_PKG_VERSION")
    )
    .unwrap();
    writeln!(
        w,
        "# HELP iqai_db_reachable Metrikler için trade DB açılabildi (1=evet)."
    )
    .unwrap();
    writeln!(w, "# TYPE iqai_db_reachable gauge").unwrap();
    writeln!(
        w,
        "iqai_db_reachable {}",
        if db_reachable { 1 } else { 0 }
    )
    .unwrap();
    let q04 = crate::binance_error::prometheus_exchange_normalized_errors();
    if !q04.is_empty() {
        w.push_str(&q04);
    }
    w
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::binance_error::{classify_binance_json, reset_exchange_normalized_metrics_for_test};
    use serde_json::json;
    use std::fs;
    use std::sync::Mutex;

    /// `render_prometheus_sli` Q04 belleğini `drain` ettiği için paralel testler birbirinin sayacını alır.
    static SLI_TEST_MUTEX: Mutex<()> = Mutex::new(());

    #[test]
    fn prometheus_export_contains_counters() {
        let _guard = SLI_TEST_MUTEX.lock().expect("sli test mutex");
        reset_exchange_normalized_metrics_for_test();
        let p = std::env::temp_dir().join(format!("iqai_sli_{}.db", uuid::Uuid::new_v4()));
        let path = p.to_str().unwrap();
        let db = TradeDb::open(Some(path)).expect("db");
        db.sli_incr("exec_order_open_success_total", 2.0).unwrap();
        let text = render_prometheus_sli(&db).expect("render");
        assert!(text.contains("exec_order_open_success_total"));
        assert!(text.contains("iqai_db_reachable 1"));
        assert!(text.contains("iqai_info"));
        drop(db);
        let _ = fs::remove_file(&p);
    }

    #[test]
    fn prometheus_export_includes_tfai_q04_normalized_errors() {
        let _guard = SLI_TEST_MUTEX.lock().expect("sli test mutex");
        reset_exchange_normalized_metrics_for_test();
        let body = json!({"code": -2015, "msg": "Invalid API-key"});
        let _ = classify_binance_json("binance_futures", 401, &body);
        let p = std::env::temp_dir().join(format!("iqai_sli_q04_{}.db", uuid::Uuid::new_v4()));
        let path = p.to_str().unwrap();
        let db = TradeDb::open(Some(path)).expect("db");
        let text = render_prometheus_sli(&db).expect("render");
        assert!(
            text.contains("iqai_exchange_normalized_errors_total"),
            "{}",
            text
        );
        assert!(text.contains("auth_failure"));
        drop(db);
        let _ = fs::remove_file(&p);
    }
}
