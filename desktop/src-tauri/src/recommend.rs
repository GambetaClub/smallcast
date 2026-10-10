//! Local, on-device recommendation engine for the palette.
//!
//! The user's `analytics.jsonl` log is the only training signal. Every
//! `execute` event is a positive example: at that moment, in that context
//! (time-of-day, day-of-week, what was used recently), the user picked
//! this item over everything else available.
//!
//! We learn a tiny logistic regression — seven weights — over per-item
//! features that capture the obvious signals:
//!
//!   f0  bias
//!   f1  log(1 + exec_count[item])              — frequency
//!   f2  exp(-Δhours[item] / HALFLIFE_HOURS)    — recency
//!   f3  log P(bucket(now) | item)               — time-of-day affinity
//!   f4  log P(weekday(now) | item)              — day-of-week affinity
//!   f5  used_within_30min                        — same-session bonus
//!   f6  log(1 + kind_count[kind(item)])          — kind prior
//!
//! Training walks the log chronologically. For each execute event we
//! freeze a snapshot of the feature space *as of just before* the event,
//! sample N negatives from items seen previously, and do pairwise
//! logistic SGD updates pushing score(positive) above score(negative).
//! Six floats fit anywhere; the model retrains in well under a second
//! for tens of thousands of events.
//!
//! No network, no telemetry, no model upload — same posture as
//! `analytics.rs`. State lives at
//! `~/Library/Application Support/smallcast/recommender_state.json`.

use chrono::{DateTime, Datelike, Timelike, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Number of weights in the model. Keep in sync with `featurize`.
pub const N_FEATURES: usize = 7;

/// Hours after which the recency feature has decayed to 0.5. Roughly a
/// week — long enough that something used last Tuesday still carries
/// weight on Friday.
const HALFLIFE_HOURS: f32 = 168.0;

/// Laplace smoothing for time-bucket / dow-bucket conditional probabilities.
/// Keeps `log(p)` finite and prevents a single execute from declaring
/// "this item is ONLY ever used at 3am."
const SMOOTH_ALPHA: f32 = 1.0;

/// Number of negative samples drawn per positive example during training.
const NEGATIVES_PER_POSITIVE: usize = 4;

/// SGD learning rate. Small because we do many passes (one per execute,
/// across several epochs) and we don't want the model thrashing on the
/// last batch in the file.
const LEARNING_RATE: f32 = 0.05;

/// L2 regularization coefficient. Keeps weights from blowing up when one
/// feature happens to dominate (e.g. recency right after a session).
const L2_LAMBDA: f32 = 0.005;

/// Number of full passes over the snapshot set during training. With one
/// epoch the SGD pass is order-dependent — early events that have no
/// valid negatives never produce updates, and a later cluster can pull
/// the weights in the wrong direction. Five epochs over a shuffled
/// snapshot list gives clean convergence on real-shaped data without
/// being slow.
const TRAINING_EPOCHS: usize = 5;

/// "Within session" window for the f5 feature. Anything used in the last
/// 30 minutes counts as same-session-recent.
const SESSION_WINDOW_MS: u128 = 30 * 60 * 1000;

/// Minimum executes the user must have logged before we treat the model
/// as trained at all. Below this we just return uniform 0.5.
const MIN_TRAIN_EVENTS: usize = 20;

/// Time-of-day buckets, used for both f3 (item-conditional) and the
/// "key" stored in per-item stats.
#[inline]
fn tod_bucket(dt: &DateTime<Utc>) -> usize {
    // Local hour — analytics.jsonl stores UTC ms, so we run buckets in UTC.
    // For a single-user app the offset is constant; the bucket boundary
    // shifts by their offset, which is fine for "morning vs evening".
    let h = dt.hour();
    if (5..12).contains(&h) {
        0 // morning
    } else if (12..17).contains(&h) {
        1 // afternoon
    } else if (17..22).contains(&h) {
        2 // evening
    } else {
        3 // night
    }
}
const N_TOD_BUCKETS: usize = 4;

#[inline]
fn dow_bucket(dt: &DateTime<Utc>) -> usize {
    // 0..=4 (Mon..Fri) -> weekday; 5..=6 -> weekend.
    let wd = dt.weekday().num_days_from_monday();
    if wd < 5 { 0 } else { 1 }
}
const N_DOW_BUCKETS: usize = 2;

/// Stable identity for a palette entry — matches the frontend's `entryKey`
/// shape so feedback ("user clicked this") aligns with scoring lookup.
/// We serialize as one string so it doubles as a HashMap key.
pub type ItemKey = String;

/// Per-item training state. Maintained while replaying the log; also
/// what the inference path uses to score "current candidates against
/// learned weights."
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ItemStats {
    /// How many times this item has been executed.
    pub exec_count: u32,
    /// Unix ms of the last execute. Drives the recency feature.
    pub last_used_ms: u128,
    /// Per-TOD-bucket execute counts.
    pub tod_counts: [u32; N_TOD_BUCKETS],
    /// Per-DOW-bucket execute counts.
    pub dow_counts: [u32; N_DOW_BUCKETS],
    /// Item kind ("app", "snippet", "quicklink", ...). Used for the
    /// kind-prior feature. Always taken from the most recent observation.
    pub kind: String,
}

/// Fully serialized model state. One JSON file on disk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecommenderState {
    /// Logistic regression weights, in feature order.
    pub weights: [f32; N_FEATURES],
    /// Per-item training stats (counts, last-used). Keyed by item key.
    pub items: HashMap<ItemKey, ItemStats>,
    /// Per-kind execute counts. Drives f6.
    pub kind_counts: HashMap<String, u32>,
    /// Total number of execute events the model has been trained on.
    pub train_examples: usize,
    /// Unix ms of the last execute event the model has *seen*. Used to
    /// short-circuit retrain when nothing new has happened.
    pub last_event_ms: u128,
    /// Unix ms when training last completed.
    pub trained_at_ms: u128,
}

impl Default for RecommenderState {
    fn default() -> Self {
        Self {
            weights: [0.0; N_FEATURES],
            items: HashMap::new(),
            kind_counts: HashMap::new(),
            train_examples: 0,
            last_event_ms: 0,
            trained_at_ms: 0,
        }
    }
}

// ---------- Disk I/O ----------

fn state_path() -> Option<PathBuf> {
    crate::store::Store::root_dir()
        .ok()
        .map(|p| p.join("recommender_state.json"))
}

pub fn load_state() -> RecommenderState {
    let Some(path) = state_path() else {
        return RecommenderState::default();
    };
    match std::fs::read_to_string(&path) {
        Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
        Err(_) => RecommenderState::default(),
    }
}

pub fn save_state(state: &RecommenderState) -> Result<(), String> {
    let Some(path) = state_path() else {
        return Err("no data dir".into());
    };
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let tmp = path.with_extension("json.tmp");
    let bytes = serde_json::to_vec_pretty(state).map_err(|e| e.to_string())?;
    std::fs::write(&tmp, &bytes).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &path).map_err(|e| e.to_string())?;
    Ok(())
}

// ---------- Featurization ----------

/// Compute the feature vector for an item against the given context.
///
/// `kind_total` is the total exec_count for this item's kind, used by f6.
/// `now_ms` is the timestamp we're scoring against (training: the moment
/// of the candidate event; inference: now).
fn featurize(
    stats: &ItemStats,
    kind_total: u32,
    now_ms: u128,
    now_dt: &DateTime<Utc>,
) -> [f32; N_FEATURES] {
    let mut f = [0.0f32; N_FEATURES];
    f[0] = 1.0; // bias

    // Frequency: log(1 + n).
    f[1] = (1.0 + stats.exec_count as f32).ln();

    // Recency: exp(-Δhours / halflife_hours_natural).
    // We use a halflife in the exponential-decay sense: f goes from 1.0
    // immediately after use down toward 0.0 over weeks.
    let dt_ms = now_ms.saturating_sub(stats.last_used_ms);
    let hours = (dt_ms as f32) / (1000.0 * 60.0 * 60.0);
    f[2] = (-hours / HALFLIFE_HOURS).exp();

    // Time-of-day affinity: log P(bucket | item) under Laplace smoothing.
    // Returned as a *delta* from the uniform prior, so an item with no
    // bias produces 0 here and the weight isn't fighting an arbitrary
    // baseline.
    let tod = tod_bucket(now_dt);
    let tot_tod: u32 = stats.tod_counts.iter().sum();
    let p_tod =
        (stats.tod_counts[tod] as f32 + SMOOTH_ALPHA)
            / (tot_tod as f32 + SMOOTH_ALPHA * N_TOD_BUCKETS as f32);
    f[3] = p_tod.ln() - (1.0 / N_TOD_BUCKETS as f32).ln();

    // Day-of-week affinity, same shape.
    let dow = dow_bucket(now_dt);
    let tot_dow: u32 = stats.dow_counts.iter().sum();
    let p_dow =
        (stats.dow_counts[dow] as f32 + SMOOTH_ALPHA)
            / (tot_dow as f32 + SMOOTH_ALPHA * N_DOW_BUCKETS as f32);
    f[4] = p_dow.ln() - (1.0 / N_DOW_BUCKETS as f32).ln();

    // Same-session bonus.
    f[5] = if dt_ms > 0 && (dt_ms as u128) <= SESSION_WINDOW_MS { 1.0 } else { 0.0 };

    // Kind prior: how popular is this *kind* of thing in general?
    f[6] = (1.0 + kind_total as f32).ln();

    f
}

#[inline]
fn dot(w: &[f32; N_FEATURES], f: &[f32; N_FEATURES]) -> f32 {
    let mut s = 0.0;
    for i in 0..N_FEATURES {
        s += w[i] * f[i];
    }
    s
}

#[inline]
fn sigmoid(x: f32) -> f32 {
    if x >= 0.0 {
        1.0 / (1.0 + (-x).exp())
    } else {
        let e = x.exp();
        e / (1.0 + e)
    }
}

// ---------- Training ----------

/// Read every event from the analytics log, in order, and replay it.
///
/// Returns the trained state (replaces whatever was on disk). Callers
/// that want to persist must pass it to `save_state`.
pub fn train_from_log() -> Result<RecommenderState, String> {
    let Some(path) = crate::analytics::log_path() else {
        return Err("no data dir".into());
    };
    let content = std::fs::read_to_string(&path).unwrap_or_default();
    train_from_jsonl(&content)
}

/// Pure version — takes the JSONL content as a string. The disk-touching
/// `train_from_log` is a thin wrapper. Tested separately.
pub fn train_from_jsonl(content: &str) -> Result<RecommenderState, String> {
    let mut state = RecommenderState::default();

    // Materialize and sort events by timestamp so we replay in true order
    // even if the file (rare) was concatenated out of order from a backup.
    let mut events: Vec<(u128, String, String, serde_json::Value)> = Vec::new();
    for line in content.lines() {
        let Ok(ev) = serde_json::from_str::<serde_json::Value>(line) else { continue };
        let ts = ev.get("ts").and_then(|v| v.as_u64()).map(|t| t as u128).unwrap_or(0);
        let kind = ev.get("kind").and_then(|v| v.as_str()).unwrap_or("").to_string();
        if kind != "execute" {
            continue;
        }
        let data = ev.get("data").cloned().unwrap_or(serde_json::Value::Null);
        // Only count successful, non-command, non-clipboard executions —
        // these match the same gate the palette uses for `touchRecent`.
        if data.get("success").and_then(|v| v.as_bool()) != Some(true) {
            continue;
        }
        let item_kind = data
            .get("kind")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if item_kind == "command" || item_kind == "clipboard" || item_kind.is_empty() {
            continue;
        }
        let name = data
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let key = format!("{}:{}", item_kind, name);
        events.push((ts, key, item_kind, data));
    }
    events.sort_by_key(|e| e.0);

    if events.len() < MIN_TRAIN_EVENTS {
        // Not enough data — return defaults, surface count for UI.
        state.train_examples = events.len();
        state.last_event_ms = events.last().map(|e| e.0).unwrap_or(0);
        state.trained_at_ms = now_ms();
        // Still record per-item stats so the empty-query "recents bias"
        // can use them even before the model is confident.
        for (ts, key, kind, _data) in &events {
            update_item_stats(&mut state, key, kind, *ts);
        }
        return Ok(state);
    }

    // Training in two phases:
    //   1) Walk events chronologically. At each event, snapshot the
    //      positive feature vector using state-up-to-but-not-including
    //      the event, then update state. We also snapshot up to N
    //      "real" negatives drawn from history at that moment plus a
    //      synthetic "zero-stats" negative so even early events (when
    //      history is empty) produce a valid training pair.
    //   2) Do TRAINING_EPOCHS shuffled passes of pairwise SGD over the
    //      snapshot list. Shuffling decouples the result from event
    //      order, which matters when usage is bursty (e.g. all-iTerm
    //      morning, all-Slack evening — without shuffling the last
    //      cluster pulls the weights toward itself).
    type Snapshot = (
        [f32; N_FEATURES],         // positive
        Vec<[f32; N_FEATURES]>,    // negatives, including a synthetic zero
    );
    let mut snapshots: Vec<Snapshot> = Vec::with_capacity(events.len());
    let mut seen_keys: Vec<ItemKey> = Vec::new();
    // Cheap deterministic RNG so tests are stable.
    let mut rng_state: u64 = 0x9E3779B97F4A7C15;
    // Synthetic "unknown / zero-history" item — every positive should
    // beat this. Constant per-event because the time-of-day affinity
    // contributes 0 (Laplace smoothed uniform) under zero counts.
    let zero_stats = ItemStats::default();

    for (ts, key, kind, _data) in &events {
        let now_ms = *ts;
        let now_dt = match DateTime::<Utc>::from_timestamp((now_ms / 1000) as i64, 0)
        {
            Some(d) => d,
            None => continue,
        };

        let pos_stats = state.items.get(key).cloned().unwrap_or_else(|| {
            let mut s = ItemStats::default();
            s.kind = kind.clone();
            s
        });
        let pos_kind_total = *state.kind_counts.get(kind).unwrap_or(&0);
        let pos_features = featurize(&pos_stats, pos_kind_total, now_ms, &now_dt);

        let mut negatives: Vec<[f32; N_FEATURES]> = Vec::new();
        // Synthetic "unknown item" baseline.
        negatives.push(featurize(&zero_stats, 0, now_ms, &now_dt));
        // Real items from history, when available.
        if seen_keys.len() >= 2 {
            for _ in 0..NEGATIVES_PER_POSITIVE {
                rng_state = next_rand(rng_state);
                let idx = (rng_state as usize) % seen_keys.len();
                let neg_key = &seen_keys[idx];
                if neg_key == key {
                    continue;
                }
                let neg_stats = match state.items.get(neg_key) {
                    Some(s) => s.clone(),
                    None => continue,
                };
                let neg_kind_total =
                    *state.kind_counts.get(&neg_stats.kind).unwrap_or(&0);
                negatives.push(featurize(
                    &neg_stats,
                    neg_kind_total,
                    now_ms,
                    &now_dt,
                ));
            }
        }
        snapshots.push((pos_features, negatives));

        if !seen_keys.contains(key) {
            seen_keys.push(key.clone());
        }
        update_item_stats(&mut state, key, kind, now_ms);
        state.train_examples += 1;
        state.last_event_ms = now_ms;
    }

    // Phase 2: shuffled SGD over the snapshot set.
    let mut order: Vec<usize> = (0..snapshots.len()).collect();
    for _epoch in 0..TRAINING_EPOCHS {
        // Fisher–Yates shuffle using the same xorshift stream.
        for i in (1..order.len()).rev() {
            rng_state = next_rand(rng_state);
            let j = (rng_state as usize) % (i + 1);
            order.swap(i, j);
        }
        for &idx in &order {
            let (pos, negs) = &snapshots[idx];
            for neg in negs {
                pairwise_sgd_update(&mut state.weights, pos, neg);
            }
        }
    }

    state.trained_at_ms = now_ms();
    Ok(state)
}

/// Apply one pairwise logistic SGD update: encourage `pos` to outscore `neg`.
///
/// Loss for one pair = -log(sigmoid(score(pos) - score(neg))) + λ‖w‖².
/// Gradient w.r.t. weights w is `(σ(s_n - s_p)) * (f_n - f_p) + 2λ * w`.
fn pairwise_sgd_update(
    w: &mut [f32; N_FEATURES],
    f_pos: &[f32; N_FEATURES],
    f_neg: &[f32; N_FEATURES],
) {
    let s_pos = dot(w, f_pos);
    let s_neg = dot(w, f_neg);
    let p = sigmoid(s_neg - s_pos); // P(model is wrong)
    for i in 0..N_FEATURES {
        let grad = p * (f_neg[i] - f_pos[i]) + 2.0 * L2_LAMBDA * w[i];
        w[i] -= LEARNING_RATE * grad;
    }
}

fn update_item_stats(
    state: &mut RecommenderState,
    key: &str,
    kind: &str,
    ts_ms: u128,
) {
    let dt = match DateTime::<Utc>::from_timestamp((ts_ms / 1000) as i64, 0) {
        Some(d) => d,
        None => return,
    };
    let tod = tod_bucket(&dt);
    let dow = dow_bucket(&dt);
    let entry = state
        .items
        .entry(key.to_string())
        .or_insert_with(|| {
            let mut s = ItemStats::default();
            s.kind = kind.to_string();
            s
        });
    entry.exec_count = entry.exec_count.saturating_add(1);
    entry.last_used_ms = ts_ms;
    entry.tod_counts[tod] = entry.tod_counts[tod].saturating_add(1);
    entry.dow_counts[dow] = entry.dow_counts[dow].saturating_add(1);
    if !kind.is_empty() {
        entry.kind = kind.to_string();
    }
    let kc = state.kind_counts.entry(kind.to_string()).or_insert(0);
    *kc = kc.saturating_add(1);
}

/// xorshift* — small deterministic RNG. Plenty for "pick a random
/// negative item." Avoids pulling in `rand` for one call site.
#[inline]
fn next_rand(mut x: u64) -> u64 {
    x ^= x >> 12;
    x ^= x << 25;
    x ^= x >> 27;
    x.wrapping_mul(0x2545F4914F6CDD1D)
}

// ---------- Inference ----------

/// One scoring request from the frontend.
#[derive(Debug, Clone, Deserialize)]
pub struct ScoreInput {
    /// Stable key produced by the frontend's `entryKey`. Same shape we
    /// log to analytics so the model finds matching history.
    pub key: String,
    /// Kind ("app" / "snippet" / etc.). Used for the kind-prior feature
    /// when the item has no history of its own.
    pub kind: String,
}

/// One scored item.
#[derive(Debug, Clone, Serialize)]
pub struct ScoreOutput {
    pub key: String,
    /// Sigmoid output in [0, 1]. Higher = more recommended.
    pub score: f32,
    /// True when this item has historical executes — purely informational
    /// so the frontend can tag a "no history yet" hint if it wants.
    pub has_history: bool,
}

/// Score a list of candidate items against the current time. Cheap —
/// O(n) over candidates, single pass, no allocation per item beyond the
/// tiny output.
pub fn score_items(state: &RecommenderState, inputs: &[ScoreInput]) -> Vec<ScoreOutput> {
    score_items_at(state, inputs, now_ms())
}

/// Lower-level variant: score against a caller-supplied timestamp.
/// Used by tests so output isn't sensitive to wall-clock time.
pub fn score_items_at(
    state: &RecommenderState,
    inputs: &[ScoreInput],
    now_ms: u128,
) -> Vec<ScoreOutput> {
    let now_dt = match DateTime::<Utc>::from_timestamp((now_ms / 1000) as i64, 0) {
        Some(d) => d,
        None => Utc::now(),
    };
    inputs
        .iter()
        .map(|inp| {
            let stats = state.items.get(&inp.key).cloned().unwrap_or_else(|| {
                let mut s = ItemStats::default();
                s.kind = inp.kind.clone();
                s
            });
            let kind_total = *state.kind_counts.get(&inp.kind).unwrap_or(&0);
            let f = featurize(&stats, kind_total, now_ms, &now_dt);
            let s = sigmoid(dot(&state.weights, &f));
            ScoreOutput {
                key: inp.key.clone(),
                score: s,
                has_history: stats.exec_count > 0,
            }
        })
        .collect()
}

// ---------- Status ----------

/// Frontend-friendly summary. Everything the Preferences view needs to
/// render the model's state, plus a peek at what the model thinks are
/// the user's top items right now.
#[derive(Debug, Clone, Serialize)]
pub struct RecommenderStatus {
    pub trained: bool,
    pub trained_at_ms: u128,
    pub last_event_ms: u128,
    pub train_examples: usize,
    pub item_count: usize,
    pub weights: [f32; N_FEATURES],
    pub feature_names: [&'static str; N_FEATURES],
    pub top_items: Vec<TopItem>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TopItem {
    pub key: String,
    pub kind: String,
    pub score: f32,
    pub exec_count: u32,
    pub last_used_ms: u128,
}

pub fn status(state: &RecommenderState) -> RecommenderStatus {
    let trained = state.train_examples >= MIN_TRAIN_EVENTS;
    let now_ms = now_ms();
    let now_dt = DateTime::<Utc>::from_timestamp((now_ms / 1000) as i64, 0)
        .unwrap_or_else(Utc::now);

    let mut top: Vec<TopItem> = state
        .items
        .iter()
        .map(|(key, stats)| {
            let kind_total = *state.kind_counts.get(&stats.kind).unwrap_or(&0);
            let f = featurize(stats, kind_total, now_ms, &now_dt);
            let score = sigmoid(dot(&state.weights, &f));
            TopItem {
                key: key.clone(),
                kind: stats.kind.clone(),
                score,
                exec_count: stats.exec_count,
                last_used_ms: stats.last_used_ms,
            }
        })
        .collect();
    top.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    top.truncate(10);

    RecommenderStatus {
        trained,
        trained_at_ms: state.trained_at_ms,
        last_event_ms: state.last_event_ms,
        train_examples: state.train_examples,
        item_count: state.items.len(),
        weights: state.weights,
        feature_names: [
            "bias",
            "frequency",
            "recency",
            "time_of_day",
            "day_of_week",
            "same_session",
            "kind_prior",
        ],
        top_items: top,
    }
}

fn now_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

// ---------- Tests ----------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Build an artificial JSONL log with 30 executes — enough to clear
    /// MIN_TRAIN_EVENTS — heavily biased toward one app at one time of day.
    /// Trained weights should learn that bias.
    fn synthetic_log() -> String {
        let day_ms: u128 = 1_777_161_600_000; // 2026-04-26 UTC
        let mut events: Vec<serde_json::Value> = Vec::new();
        // 25 executes of iTerm in the morning
        for i in 0..25 {
            let ts = day_ms + (i as u128) * 60_000 + 7 * 3600 * 1000; // 07:0X UTC
            events.push(json!({
                "ts": ts, "session_id": "s", "kind": "execute",
                "data": {"kind": "app", "name": "iTerm", "success": true, "q": "i"}
            }));
        }
        // 5 executes of Slack in the evening
        for i in 0..5 {
            let ts = day_ms + (i as u128) * 60_000 + 19 * 3600 * 1000; // 19:0X UTC
            events.push(json!({
                "ts": ts, "session_id": "s", "kind": "execute",
                "data": {"kind": "app", "name": "Slack", "success": true, "q": "s"}
            }));
        }
        events
            .iter()
            .map(|e| serde_json::to_string(e).unwrap())
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn empty_log_returns_default_state() {
        let s = train_from_jsonl("").unwrap();
        assert_eq!(s.train_examples, 0);
        assert_eq!(s.weights, [0.0; N_FEATURES]);
    }

    #[test]
    fn unsuccessful_or_command_events_are_skipped() {
        let day_ms: u128 = 1_777_161_600_000;
        let events = vec![
            json!({"ts": day_ms, "session_id": "s", "kind": "execute",
                "data": {"kind": "command", "name": "Open Preferences", "success": true}}),
            json!({"ts": day_ms + 1, "session_id": "s", "kind": "execute",
                "data": {"kind": "app", "name": "iTerm", "success": false}}),
            json!({"ts": day_ms + 2, "session_id": "s", "kind": "execute",
                "data": {"kind": "clipboard", "name": "x", "success": true}}),
        ];
        let jsonl = events
            .iter()
            .map(|e| serde_json::to_string(e).unwrap())
            .collect::<Vec<_>>()
            .join("\n");
        let s = train_from_jsonl(&jsonl).unwrap();
        assert_eq!(s.train_examples, 0);
        assert!(s.items.is_empty());
    }

    /// Fixed inference timestamp for tests — 2026-04-26 07:30 UTC,
    /// matching the synthetic log's morning iTerm bucket so the
    /// time-of-day feature isn't penalising iTerm.
    const TEST_NOW_MS: u128 = 1_777_161_600_000 + 7 * 3_600_000 + 30 * 60_000;

    #[test]
    fn training_assigns_higher_score_to_dominant_item() {
        // After training on a log dominated by iTerm, a fresh score for
        // iTerm should beat Slack at iTerm's typical time of day.
        let s = train_from_jsonl(&synthetic_log()).unwrap();
        assert!(s.train_examples >= MIN_TRAIN_EVENTS);
        let inputs = vec![
            ScoreInput { key: "app:iTerm".into(), kind: "app".into() },
            ScoreInput { key: "app:Slack".into(), kind: "app".into() },
        ];
        let scores = score_items_at(&s, &inputs, TEST_NOW_MS);
        let iterm = scores.iter().find(|x| x.key == "app:iTerm").unwrap();
        let slack = scores.iter().find(|x| x.key == "app:Slack").unwrap();
        assert!(
            iterm.score > slack.score,
            "iTerm ({}) should outrank Slack ({}) after training",
            iterm.score,
            slack.score
        );
    }

    #[test]
    fn time_of_day_signal_flips_ranking_at_evening() {
        // Inverse of the above: at Slack's typical evening time, the
        // model should weight TOD enough that Slack pulls ahead despite
        // its much lower frequency. This is the whole point of the
        // feature — we want context-aware ranking, not pure popularity.
        let s = train_from_jsonl(&synthetic_log()).unwrap();
        let evening = 1_777_161_600_000 + 19 * 3_600_000 + 30 * 60_000;
        let inputs = vec![
            ScoreInput { key: "app:iTerm".into(), kind: "app".into() },
            ScoreInput { key: "app:Slack".into(), kind: "app".into() },
        ];
        let scores = score_items_at(&s, &inputs, evening);
        let iterm = scores.iter().find(|x| x.key == "app:iTerm").unwrap();
        let slack = scores.iter().find(|x| x.key == "app:Slack").unwrap();
        assert!(
            slack.score > iterm.score,
            "at evening Slack ({}) should beat iTerm ({}) on TOD bias",
            slack.score,
            iterm.score
        );
    }

    #[test]
    fn item_with_no_history_scores_lower_than_frequent_item() {
        let s = train_from_jsonl(&synthetic_log()).unwrap();
        let inputs = vec![
            ScoreInput { key: "app:iTerm".into(), kind: "app".into() },
            ScoreInput { key: "app:NeverSeen".into(), kind: "app".into() },
        ];
        let scores = score_items_at(&s, &inputs, TEST_NOW_MS);
        let known = scores.iter().find(|x| x.key == "app:iTerm").unwrap();
        let unknown = scores.iter().find(|x| x.key == "app:NeverSeen").unwrap();
        assert!(known.has_history);
        assert!(!unknown.has_history);
        assert!(
            known.score > unknown.score,
            "iTerm ({}) should outrank NeverSeen ({}) at iTerm's typical TOD",
            known.score,
            unknown.score
        );
    }

    #[test]
    fn recency_decays_score_for_old_items() {
        // Build a state where one item was used right now and another a
        // year ago, then compare scores. Frequencies equal, only recency
        // should differ.
        let mut state = RecommenderState::default();
        state.weights = [0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0]; // pure recency
        let now = now_ms();
        let year = 365u128 * 24 * 3600 * 1000;
        state.items.insert(
            "app:Fresh".into(),
            ItemStats { exec_count: 5, last_used_ms: now, kind: "app".into(), ..Default::default() },
        );
        state.items.insert(
            "app:Stale".into(),
            ItemStats { exec_count: 5, last_used_ms: now.saturating_sub(year), kind: "app".into(), ..Default::default() },
        );
        let scores = score_items(&state, &[
            ScoreInput { key: "app:Fresh".into(), kind: "app".into() },
            ScoreInput { key: "app:Stale".into(), kind: "app".into() },
        ]);
        assert!(scores[0].score > scores[1].score);
    }

    #[test]
    fn weights_change_after_training() {
        let s = train_from_jsonl(&synthetic_log()).unwrap();
        assert_ne!(s.weights, [0.0; N_FEATURES],
            "training should have moved at least one weight off zero");
        // Frequency weight (index 1) should be positive — items used more
        // often beat items used less often.
        assert!(
            s.weights[1] > 0.0,
            "frequency weight should be positive after training, got {}",
            s.weights[1]
        );
    }

    #[test]
    fn status_reports_top_items_in_score_order() {
        let s = train_from_jsonl(&synthetic_log()).unwrap();
        let st = status(&s);
        assert!(st.trained);
        assert!(st.train_examples >= MIN_TRAIN_EVENTS);
        // Top item should be the dominant one; score should be a
        // probability in [0, 1].
        assert!(!st.top_items.is_empty());
        for tw in st.top_items.windows(2) {
            assert!(tw[0].score >= tw[1].score);
        }
        for it in &st.top_items {
            assert!(it.score >= 0.0 && it.score <= 1.0);
        }
    }

    #[test]
    fn tod_buckets_cover_all_24_hours() {
        // Sanity: every hour maps to exactly one bucket and the four
        // boundaries don't overlap.
        let mut counts = [0u32; N_TOD_BUCKETS];
        let day = chrono::DateTime::<Utc>::from_timestamp(0, 0).unwrap();
        for h in 0..24 {
            let dt = day + chrono::Duration::hours(h);
            counts[tod_bucket(&dt)] += 1;
        }
        assert_eq!(counts.iter().sum::<u32>(), 24);
    }

    #[test]
    fn small_log_below_min_still_records_item_stats() {
        // 3 executes < MIN_TRAIN_EVENTS — model isn't trained, but item
        // counts must still accumulate so the empty-query bias works.
        let day_ms: u128 = 1_777_161_600_000;
        let events: Vec<serde_json::Value> = (0..3)
            .map(|i| json!({
                "ts": day_ms + i as u128, "session_id": "s", "kind": "execute",
                "data": {"kind": "app", "name": "iTerm", "success": true}
            }))
            .collect();
        let jsonl = events
            .iter()
            .map(|e| serde_json::to_string(e).unwrap())
            .collect::<Vec<_>>()
            .join("\n");
        let s = train_from_jsonl(&jsonl).unwrap();
        assert_eq!(s.train_examples, 3);
        assert_eq!(s.items.get("app:iTerm").unwrap().exec_count, 3);
        assert_eq!(s.weights, [0.0; N_FEATURES],
            "should not have trained weights below MIN_TRAIN_EVENTS");
    }

    // ---------- End-to-end demo ----------
    //
    // This test is `#[ignore]` so it doesn't fire on `cargo test` by
    // default. Run it with:
    //
    //   cargo test --manifest-path src-tauri/Cargo.toml -p desktop \
    //       recommender_demo -- --ignored --nocapture
    //
    // It builds a realistic two-week usage log for an imaginary
    // developer, trains the model, and prints what the model learned
    // and how it ranks the catalog at three different times of day.
    // The point is to see the recommender end-to-end without booting
    // the desktop app.

    /// Emit a single execute event into the JSONL builder.
    fn ev(out: &mut Vec<String>, ts_ms: u128, kind: &str, name: &str) {
        let line = format!(
            "{{\"ts\":{},\"session_id\":\"demo\",\"kind\":\"execute\",\"data\":{{\"kind\":\"{}\",\"name\":\"{}\",\"success\":true}}}}",
            ts_ms, kind, name
        );
        out.push(line);
    }

    /// 14-day fake log shaped like a developer's actual day:
    ///   - mornings (07–11 UTC):   iTerm + Cursor heavy, occasional Chrome
    ///   - midday   (12–16 UTC):   Slack + Chrome + GitHub
    ///   - evenings (18–21 UTC):   Spotify + Slack + Chrome
    ///   - weekends:                less terminal, more Spotify/Chrome
    /// Realistic noise: ~2 random "wrong" picks per day.
    fn realistic_two_week_log() -> String {
        let day0_ms: u128 = 1_775_001_600_000; // Mon 2026-04-01 00:00 UTC
        let day_ms: u128 = 24 * 3600 * 1000;
        let mut lines: Vec<String> = Vec::new();
        for day in 0..14u128 {
            let weekend = day % 7 >= 5;
            let base = day0_ms + day * day_ms;
            // Morning slot (07:00–10:00 UTC, every 20 min).
            for slot in 0..9 {
                let t = base + 7 * 3600 * 1000 + slot * 20 * 60 * 1000;
                if weekend {
                    if slot % 3 == 0 { ev(&mut lines, t, "app", "Chrome"); }
                    else { ev(&mut lines, t, "app", "Spotify"); }
                } else {
                    match slot % 3 {
                        0 => ev(&mut lines, t, "app", "iTerm"),
                        1 => ev(&mut lines, t, "app", "Cursor"),
                        _ => ev(&mut lines, t, "app", "iTerm"),
                    }
                }
            }
            // Midday slot (12:00–16:00 UTC, every 30 min).
            for slot in 0..8 {
                let t = base + 12 * 3600 * 1000 + slot * 30 * 60 * 1000;
                match slot % 4 {
                    0 => ev(&mut lines, t, "app", "Slack"),
                    1 => ev(&mut lines, t, "app", "Chrome"),
                    2 => ev(&mut lines, t, "quicklink", "GitHub PRs"),
                    _ => ev(&mut lines, t, "app", "Slack"),
                }
            }
            // Evening slot (18:00–21:00 UTC, every 30 min).
            for slot in 0..6 {
                let t = base + 18 * 3600 * 1000 + slot * 30 * 60 * 1000;
                match slot % 3 {
                    0 => ev(&mut lines, t, "app", "Spotify"),
                    1 => ev(&mut lines, t, "app", "Slack"),
                    _ => ev(&mut lines, t, "app", "Chrome"),
                }
            }
            // Two random misclicks per day to add noise.
            ev(&mut lines, base + 9 * 3600 * 1000 + 100, "snippet", "stripe-key");
            ev(&mut lines, base + 14 * 3600 * 1000 + 100, "snippet", "stripe-key");
        }
        lines.join("\n")
    }

    fn fmt_now(ms: u128) -> String {
        let dt = DateTime::<Utc>::from_timestamp((ms / 1000) as i64, 0).unwrap();
        dt.format("%a %Y-%m-%d %H:%M UTC").to_string()
    }

    #[test]
    #[ignore]
    fn recommender_demo() {
        let jsonl = realistic_two_week_log();
        let event_count = jsonl.lines().count();
        let state = train_from_jsonl(&jsonl).unwrap();

        println!();
        println!("================================================================");
        println!(" smallcast recommender — end-to-end demo");
        println!("================================================================");
        println!();
        println!("Synthetic log:");
        println!("  events written:        {}", event_count);
        println!("  events trained on:     {}", state.train_examples);
        println!("  unique items:          {}", state.items.len());
        println!("  last event:            {}", fmt_now(state.last_event_ms));
        println!();

        println!("Learned weights (positive = pushes score up):");
        let names = [
            "bias", "frequency", "recency", "time_of_day", "day_of_week",
            "same_session", "kind_prior",
        ];
        for (i, name) in names.iter().enumerate() {
            println!("  {:<14} {:>+8.3}", name, state.weights[i]);
        }
        println!();

        // Sample exec counts so we know what was actually used vs noise.
        let mut by_count: Vec<(&String, u32)> = state
            .items
            .iter()
            .map(|(k, v)| (k, v.exec_count))
            .collect();
        by_count.sort_by(|a, b| b.1.cmp(&a.1));
        println!("Items by raw execute count:");
        for (k, n) in by_count.iter().take(8) {
            println!("  {:<26} {:>4}×", k, n);
        }
        println!();

        // Score the same catalog at three reference times: weekday
        // morning (08:00 UTC), weekday midday (14:00 UTC), and weekday
        // evening (19:00 UTC). 2026-04-15 is a Wednesday.
        let inputs: Vec<ScoreInput> = state
            .items
            .iter()
            .map(|(k, v)| ScoreInput { key: k.clone(), kind: v.kind.clone() })
            .collect();
        let mut inputs_sorted = inputs.clone();
        inputs_sorted.sort_by(|a, b| a.key.cmp(&b.key));

        let scenarios: [(&str, u128); 3] = [
            (
                "Wednesday MORNING (08:00 UTC)",
                1_775_001_600_000 + 14 * day_ms() + 8 * 3600_000,
            ),
            (
                "Wednesday MIDDAY  (14:00 UTC)",
                1_775_001_600_000 + 14 * day_ms() + 14 * 3600_000,
            ),
            (
                "Wednesday EVENING (19:00 UTC)",
                1_775_001_600_000 + 14 * day_ms() + 19 * 3600_000,
            ),
        ];
        for (label, ts) in scenarios.iter() {
            let mut scored = score_items_at(&state, &inputs_sorted, *ts);
            scored.sort_by(|a, b| {
                b.score
                    .partial_cmp(&a.score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            println!("--- {} ---", label);
            println!("Top 6 by recommender score:");
            for s in scored.iter().take(6) {
                let stats = state.items.get(&s.key).unwrap();
                println!(
                    "  {:>5.1}%  {:<26} ({}×, last used {})",
                    s.score * 100.0,
                    s.key,
                    stats.exec_count,
                    fmt_now(stats.last_used_ms),
                );
            }
            // Where would the noise items land?
            if let Some(noise) = scored.iter().find(|s| s.key == "snippet:stripe-key") {
                let rank = scored.iter().position(|s| s.key == noise.key).unwrap_or(usize::MAX);
                println!(
                    "  ... noise check: snippet:stripe-key at rank {} ({:.1}%)",
                    rank + 1,
                    noise.score * 100.0
                );
            }
            println!();
        }

        println!("================================================================");
    }

    #[inline]
    fn day_ms() -> u128 { 24 * 3600 * 1000 }

    /// Pre-train against the user's REAL analytics log on disk and
    /// write `recommender_state.json` next to it. Used once when
    /// installing the recommender so the first palette open after
    /// launch has a warm model instead of all-0.5 scores.
    ///
    ///     cargo test --manifest-path src-tauri/Cargo.toml \
    ///         pretrain_against_real_log -- --ignored --nocapture
    ///
    /// Touches the real data dir, hence `#[ignore]`.
    #[test]
    #[ignore]
    fn pretrain_against_real_log() {
        let state = train_from_log()
            .expect("train_from_log against real analytics.jsonl");
        save_state(&state).expect("save state");
        let st = status(&state);
        println!();
        println!("Pre-trained recommender against your real analytics log:");
        println!("  events trained on:  {}", st.train_examples);
        println!("  unique items:       {}", st.item_count);
        println!("  trained:            {}", st.trained);
        println!();
        println!("Learned weights:");
        for (name, w) in st.feature_names.iter().zip(st.weights.iter()) {
            println!("  {:<14} {:>+8.3}", name, w);
        }
        println!();
        println!("Top 10 items right now (at this exact moment):");
        for it in st.top_items.iter().take(10) {
            println!(
                "  {:>5.1}%  {:<40} ({}× executed)",
                it.score * 100.0,
                it.key,
                it.exec_count
            );
        }
    }

    #[test]
    fn pairwise_sgd_moves_score_diff_in_right_direction() {
        // Sanity: one update on a clear pair (positive has higher freq)
        // should *increase* the score gap, not decrease it.
        let mut w = [0.0f32; N_FEATURES];
        let pos = [1.0, 3.0, 1.0, 0.1, 0.0, 0.0, 1.0];
        let neg = [1.0, 0.5, 0.1, 0.0, 0.0, 0.0, 0.5];
        let before = dot(&w, &pos) - dot(&w, &neg);
        for _ in 0..20 {
            pairwise_sgd_update(&mut w, &pos, &neg);
        }
        let after = dot(&w, &pos) - dot(&w, &neg);
        assert!(after > before);
    }
}
