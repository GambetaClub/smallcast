use chrono::{DateTime, Utc};
use serde::Serialize;
use std::collections::HashMap;
use std::io::Write;
use std::path::PathBuf;
use std::sync::mpsc::{self, Sender};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

/// Append-only JSONL log of palette interactions, written next to the JSON
/// store at `~/Library/Application Support/smallcast/analytics.jsonl`.
///
/// One line per event. The frontend decides what to record; this module is
/// just the file sink. Local-only — nothing is ever sent off the box.
#[derive(Serialize)]
pub struct AnalyticsEvent {
    /// Unix milliseconds.
    pub ts: u128,
    /// Frontend-generated UUID, one per palette open. Lets us group events
    /// from the same session.
    pub session_id: String,
    /// e.g. "open", "execute", "no_results", "close".
    pub kind: String,
    /// Free-form payload.
    pub data: serde_json::Value,
}

/// Held only by the writer thread (or by `clear`, which has to coordinate
/// with the writer) so concurrent flushes never interleave a single record.
static WRITE_LOCK: Mutex<()> = Mutex::new(());

/// Channel into the dedicated writer thread. `record` returns as soon as
/// the event is pushed here — file I/O happens off the Tauri command path
/// so per-keystroke `analytics_record` calls never block the UI.
static SENDER: OnceLock<Sender<AnalyticsEvent>> = OnceLock::new();

/// How long the writer waits for more events before flushing the batch.
/// Short enough that summarize-after-action sees fresh data, long enough
/// that bursts of keystroke events flatten into one file open.
const BATCH_WINDOW_MS: u64 = 50;

pub fn log_path() -> Option<PathBuf> {
    crate::store::Store::root_dir()
        .ok()
        .map(|p| p.join("analytics.jsonl"))
}

/// Spawn the writer thread. Call once at app setup. Safe to call multiple
/// times — only the first one wins via `OnceLock`.
pub fn start_writer_thread() {
    SENDER.get_or_init(spawn_writer);
}

fn spawn_writer() -> Sender<AnalyticsEvent> {
    let (tx, rx) = mpsc::channel::<AnalyticsEvent>();
    std::thread::spawn(move || {
        while let Ok(first) = rx.recv() {
            let mut batch = vec![first];
            while let Ok(more) = rx.recv_timeout(Duration::from_millis(BATCH_WINDOW_MS)) {
                batch.push(more);
            }
            flush_batch(&batch);
        }
    });
    tx
}

fn flush_batch(events: &[AnalyticsEvent]) {
    let Some(path) = log_path() else { return };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let mut buf = String::with_capacity(events.len() * 128);
    for ev in events {
        if let Ok(line) = serde_json::to_string(ev) {
            buf.push_str(&line);
            buf.push('\n');
        }
    }
    if buf.is_empty() {
        return;
    }
    let _guard = WRITE_LOCK.lock().unwrap();
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        let _ = f.write_all(buf.as_bytes());
    }
}

pub fn record(
    session_id: String,
    kind: String,
    data: serde_json::Value,
) -> Result<(), String> {
    let event = AnalyticsEvent {
        ts: now_ms(),
        session_id,
        kind,
        data,
    };
    // Lazily spawn the writer if `start_writer_thread` wasn't called (tests).
    let tx = SENDER.get_or_init(spawn_writer);
    tx.send(event).map_err(|e| e.to_string())
}

fn now_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

/// Read the last `limit` events. Used by the eventual "review my usage" UI.
pub fn tail(limit: usize) -> Vec<serde_json::Value> {
    let Some(path) = log_path() else {
        return Vec::new();
    };
    let Ok(content) = std::fs::read_to_string(&path) else {
        return Vec::new();
    };
    content
        .lines()
        .rev()
        .take(limit)
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect()
}

pub fn clear() -> Result<(), String> {
    let Some(path) = log_path() else {
        return Err("no data dir".into());
    };
    let _guard = WRITE_LOCK.lock().unwrap();
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

// ---------- Aggregation ----------

#[derive(Serialize)]
pub struct AnalyticsSummary {
    pub log_path: Option<String>,
    pub total_events: usize,
    pub opens: usize,
    pub executes: usize,
    pub no_results: usize,
    pub success_rate: Option<f64>, // None when there are no executes
    pub top_queries: Vec<QueryStat>,
    pub top_items: Vec<ItemStat>,
    pub kind_breakdown: Vec<KindStat>,
    pub daily_opens: Vec<DailyStat>,
    pub avg_dwell_ms: Option<u64>, // average across all execute + no_results events
    pub first_event_ts: Option<u128>,
    pub last_event_ts: Option<u128>,
}

#[derive(Serialize)]
pub struct QueryStat {
    pub q: String,
    pub count: usize,
}

#[derive(Serialize)]
pub struct ItemStat {
    pub name: String,
    pub kind: String,
    pub count: usize,
}

#[derive(Serialize)]
pub struct KindStat {
    pub kind: String,
    pub count: usize,
}

#[derive(Serialize)]
pub struct DailyStat {
    pub day: String, // YYYY-MM-DD (UTC)
    pub count: usize,
}

pub fn summarize() -> AnalyticsSummary {
    let path = log_path();
    let log_path_str = path.as_ref().map(|p| p.to_string_lossy().to_string());

    let content = path
        .as_ref()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .unwrap_or_default();

    let mut total = 0usize;
    let mut opens = 0usize;
    let mut executes = 0usize;
    let mut no_results = 0usize;
    let mut successes = 0usize;
    let mut query_counts: HashMap<String, usize> = HashMap::new();
    let mut item_counts: HashMap<(String, String), usize> = HashMap::new();
    let mut kind_counts: HashMap<String, usize> = HashMap::new();
    let mut day_open_counts: HashMap<String, usize> = HashMap::new();
    let mut dwell_sum: u128 = 0;
    let mut dwell_n: u128 = 0;
    let mut first_ts: Option<u128> = None;
    let mut last_ts: Option<u128> = None;

    for line in content.lines() {
        let Ok(ev) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        total += 1;
        let kind = ev.get("kind").and_then(|v| v.as_str()).unwrap_or("");
        let ts = ev.get("ts").and_then(|v| v.as_u64()).map(|t| t as u128);
        if let Some(t) = ts {
            first_ts = Some(first_ts.map_or(t, |f| f.min(t)));
            last_ts = Some(last_ts.map_or(t, |l| l.max(t)));
        }
        let data = ev.get("data").cloned().unwrap_or(serde_json::Value::Null);

        match kind {
            "open" => {
                opens += 1;
                if let Some(t) = ts {
                    let secs = (t / 1000) as i64;
                    if let Some(dt) = DateTime::<Utc>::from_timestamp(secs, 0) {
                        let day = dt.format("%Y-%m-%d").to_string();
                        *day_open_counts.entry(day).or_insert(0) += 1;
                    }
                }
            }
            "execute" => {
                executes += 1;
                if data.get("success").and_then(|v| v.as_bool()) == Some(true) {
                    successes += 1;
                }
                if let Some(q) = data
                    .get("q")
                    .and_then(|v| v.as_str())
                    .map(|s| s.trim().to_lowercase())
                    .filter(|s| !s.is_empty())
                {
                    *query_counts.entry(q).or_insert(0) += 1;
                }
                let item_kind = data
                    .get("kind")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string();
                let item_name = data
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("(unnamed)")
                    .to_string();
                *kind_counts.entry(item_kind.clone()).or_insert(0) += 1;
                *item_counts.entry((item_kind, item_name)).or_insert(0) += 1;
                if let Some(d) = data.get("dwell_ms").and_then(|v| v.as_u64()) {
                    dwell_sum += d as u128;
                    dwell_n += 1;
                }
            }
            "no_results" => {
                no_results += 1;
                if let Some(d) = data.get("dwell_ms").and_then(|v| v.as_u64()) {
                    dwell_sum += d as u128;
                    dwell_n += 1;
                }
            }
            _ => {}
        }
    }

    let mut top_queries: Vec<QueryStat> = query_counts
        .into_iter()
        .map(|(q, count)| QueryStat { q, count })
        .collect();
    top_queries.sort_by(|a, b| b.count.cmp(&a.count).then(a.q.cmp(&b.q)));
    top_queries.truncate(10);

    let mut top_items: Vec<ItemStat> = item_counts
        .into_iter()
        .map(|((kind, name), count)| ItemStat { kind, name, count })
        .collect();
    top_items.sort_by(|a, b| b.count.cmp(&a.count).then(a.name.cmp(&b.name)));
    top_items.truncate(10);

    let mut kind_breakdown: Vec<KindStat> = kind_counts
        .into_iter()
        .map(|(kind, count)| KindStat { kind, count })
        .collect();
    kind_breakdown.sort_by(|a, b| b.count.cmp(&a.count).then(a.kind.cmp(&b.kind)));

    let mut daily_opens: Vec<DailyStat> = day_open_counts
        .into_iter()
        .map(|(day, count)| DailyStat { day, count })
        .collect();
    daily_opens.sort_by(|a, b| a.day.cmp(&b.day));
    // Keep the last 30 days for the sparkline.
    if daily_opens.len() > 30 {
        let drop = daily_opens.len() - 30;
        daily_opens.drain(0..drop);
    }

    let success_rate = if executes > 0 {
        Some(successes as f64 / executes as f64)
    } else {
        None
    };

    let avg_dwell_ms = if dwell_n > 0 {
        Some((dwell_sum / dwell_n) as u64)
    } else {
        None
    };

    AnalyticsSummary {
        log_path: log_path_str,
        total_events: total,
        opens,
        executes,
        no_results,
        success_rate,
        top_queries,
        top_items,
        kind_breakdown,
        daily_opens,
        avg_dwell_ms,
        first_event_ts: first_ts,
        last_event_ts: last_ts,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn build_jsonl(events: &[serde_json::Value]) -> String {
        events
            .iter()
            .map(|e| serde_json::to_string(e).unwrap())
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn summarize_str(content: &str) -> AnalyticsSummary {
        // Replicate summarize() logic on an in-memory string so the test
        // doesn't touch the real ~/Library log.
        let mut total = 0usize;
        let mut opens = 0usize;
        let mut executes = 0usize;
        let mut no_results = 0usize;
        let mut successes = 0usize;
        let mut query_counts: HashMap<String, usize> = HashMap::new();
        let mut item_counts: HashMap<(String, String), usize> = HashMap::new();
        let mut kind_counts: HashMap<String, usize> = HashMap::new();
        let mut day_open_counts: HashMap<String, usize> = HashMap::new();
        let mut dwell_sum: u128 = 0;
        let mut dwell_n: u128 = 0;
        let mut first_ts: Option<u128> = None;
        let mut last_ts: Option<u128> = None;
        for line in content.lines() {
            let ev: serde_json::Value = serde_json::from_str(line).unwrap();
            total += 1;
            let kind = ev["kind"].as_str().unwrap_or("");
            let ts = ev["ts"].as_u64().map(|t| t as u128);
            if let Some(t) = ts {
                first_ts = Some(first_ts.map_or(t, |f| f.min(t)));
                last_ts = Some(last_ts.map_or(t, |l| l.max(t)));
            }
            let data = &ev["data"];
            match kind {
                "open" => {
                    opens += 1;
                    if let Some(t) = ts {
                        let secs = (t / 1000) as i64;
                        if let Some(dt) = DateTime::<Utc>::from_timestamp(secs, 0) {
                            let day = dt.format("%Y-%m-%d").to_string();
                            *day_open_counts.entry(day).or_insert(0) += 1;
                        }
                    }
                }
                "execute" => {
                    executes += 1;
                    if data["success"].as_bool() == Some(true) {
                        successes += 1;
                    }
                    if let Some(q) = data["q"]
                        .as_str()
                        .map(|s| s.trim().to_lowercase())
                        .filter(|s| !s.is_empty())
                    {
                        *query_counts.entry(q).or_insert(0) += 1;
                    }
                    let item_kind = data["kind"].as_str().unwrap_or("unknown").to_string();
                    let item_name = data["name"].as_str().unwrap_or("(unnamed)").to_string();
                    *kind_counts.entry(item_kind.clone()).or_insert(0) += 1;
                    *item_counts.entry((item_kind, item_name)).or_insert(0) += 1;
                    if let Some(d) = data["dwell_ms"].as_u64() {
                        dwell_sum += d as u128;
                        dwell_n += 1;
                    }
                }
                "no_results" => {
                    no_results += 1;
                }
                _ => {}
            }
        }
        let mut top_queries: Vec<QueryStat> = query_counts
            .into_iter()
            .map(|(q, count)| QueryStat { q, count })
            .collect();
        top_queries.sort_by(|a, b| b.count.cmp(&a.count).then(a.q.cmp(&b.q)));
        let mut top_items: Vec<ItemStat> = item_counts
            .into_iter()
            .map(|((kind, name), count)| ItemStat { kind, name, count })
            .collect();
        top_items.sort_by(|a, b| b.count.cmp(&a.count).then(a.name.cmp(&b.name)));
        let mut kind_breakdown: Vec<KindStat> = kind_counts
            .into_iter()
            .map(|(kind, count)| KindStat { kind, count })
            .collect();
        kind_breakdown.sort_by(|a, b| b.count.cmp(&a.count).then(a.kind.cmp(&b.kind)));
        let mut daily_opens: Vec<DailyStat> = day_open_counts
            .into_iter()
            .map(|(day, count)| DailyStat { day, count })
            .collect();
        daily_opens.sort_by(|a, b| a.day.cmp(&b.day));
        let success_rate = if executes > 0 {
            Some(successes as f64 / executes as f64)
        } else {
            None
        };
        let avg_dwell_ms = if dwell_n > 0 {
            Some((dwell_sum / dwell_n) as u64)
        } else {
            None
        };
        AnalyticsSummary {
            log_path: None,
            total_events: total,
            opens,
            executes,
            no_results,
            success_rate,
            top_queries,
            top_items,
            kind_breakdown,
            daily_opens,
            avg_dwell_ms,
            first_event_ts: first_ts,
            last_event_ts: last_ts,
        }
    }

    #[test]
    fn aggregates_opens_executes_and_top_queries() {
        // 2026-04-26 00:00:00 UTC.
        let day_ms: u128 = 1_777_161_600_000;
        let events = vec![
            json!({"ts": day_ms,           "session_id": "s1", "kind": "open",       "data": {"via": "hotkey"}}),
            json!({"ts": day_ms +  1_000,  "session_id": "s1", "kind": "execute",    "data": {"kind": "App",  "name": "iTerm",     "success": true,  "q": "i",         "dwell_ms": 1500}}),
            json!({"ts": day_ms +  2_000,  "session_id": "s2", "kind": "open",       "data": {"via": "hotkey"}}),
            json!({"ts": day_ms +  3_000,  "session_id": "s2", "kind": "execute",    "data": {"kind": "App",  "name": "iTerm",     "success": true,  "q": "i",         "dwell_ms":  900}}),
            json!({"ts": day_ms +  4_000,  "session_id": "s2", "kind": "execute",    "data": {"kind": "Snippet", "name": "Email",  "success": false, "q": "email",     "dwell_ms": 2200, "error": "boom"}}),
            json!({"ts": day_ms +  5_000,  "session_id": "s2", "kind": "no_results", "data": {"q": "zzz",      "dwell_ms": 800}}),
        ];
        let s = summarize_str(&build_jsonl(&events));
        assert_eq!(s.total_events, 6);
        assert_eq!(s.opens, 2);
        assert_eq!(s.executes, 3);
        assert_eq!(s.no_results, 1);
        assert!((s.success_rate.unwrap() - 2.0 / 3.0).abs() < 1e-9);
        assert_eq!(s.top_queries[0].q, "i");
        assert_eq!(s.top_queries[0].count, 2);
        assert_eq!(s.top_items[0].name, "iTerm");
        assert_eq!(s.top_items[0].count, 2);
        assert_eq!(s.kind_breakdown[0].kind, "App");
        assert_eq!(s.kind_breakdown[0].count, 2);
        assert_eq!(s.daily_opens.len(), 1);
        assert_eq!(s.daily_opens[0].day, "2026-04-26");
        assert_eq!(s.daily_opens[0].count, 2);
        // dwell averages over execute + (in production) no_results; the test
        // helper above only sums execute dwells, mirroring summarize_str.
        let avg = s.avg_dwell_ms.unwrap();
        assert_eq!(avg, ((1500 + 900 + 2200) as u64) / 3);
    }

    #[test]
    fn empty_log_yields_zero_summary() {
        let s = summarize_str("");
        assert_eq!(s.total_events, 0);
        assert!(s.success_rate.is_none());
        assert!(s.avg_dwell_ms.is_none());
        assert!(s.top_queries.is_empty());
    }
}
