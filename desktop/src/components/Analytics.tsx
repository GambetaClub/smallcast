import { useCallback, useEffect, useState } from "react";
import { api } from "../api";
import type { AnalyticsSummary } from "../types";
import { fmtKind, fmtMs, fmtPct } from "../utils";
import { Screen } from "./Screen";

type Props = {
  onClose: () => void;
  onError: (msg: string) => void;
};

function fmtTs(ms: number | null): string {
  if (!ms) return "—";
  return new Date(ms).toLocaleString();
}

export function Analytics({ onClose, onError }: Props) {
  const [data, setData] = useState<AnalyticsSummary | null>(null);
  const [loading, setLoading] = useState(true);

  const refresh = useCallback(async () => {
    setLoading(true);
    try {
      const s = await api.analyticsSummary();
      setData(s);
    } catch (e) {
      onError(String(e));
    } finally {
      setLoading(false);
    }
  }, [onError]);

  useEffect(() => {
    refresh();
  }, [refresh]);

  async function clearLog() {
    if (
      !window.confirm(
        "Delete the analytics log file? This wipes your usage history. Cannot be undone.",
      )
    ) {
      return;
    }
    try {
      await api.analyticsClear();
      await refresh();
    } catch (e) {
      onError(String(e));
    }
  }

  const maxKind = data?.kind_breakdown[0]?.count ?? 0;
  const maxQuery = data?.top_queries[0]?.count ?? 0;
  const maxItem = data?.top_items[0]?.count ?? 0;
  const maxDay = data?.daily_opens.reduce((m, d) => Math.max(m, d.count), 0) ?? 0;

  return (
    <Screen kind="analytics" title="Analytics · smallcast" onClose={onClose}>
      <div className="analytics">
          {loading && <div className="analytics-empty">Loading…</div>}

          {!loading && data && data.total_events === 0 && (
            <div className="analytics-empty">
              No events yet. Open the palette, run something, and the log will
              start filling up at{" "}
              <code>{data.log_path ?? "~/Library/Application Support/smallcast/analytics.jsonl"}</code>
              .
            </div>
          )}

          {!loading && data && data.total_events > 0 && (
            <>
              <section className="analytics-stats">
                <div className="stat">
                  <div className="stat-label">opens</div>
                  <div className="stat-value">{data.opens}</div>
                </div>
                <div className="stat">
                  <div className="stat-label">executes</div>
                  <div className="stat-value">{data.executes}</div>
                </div>
                <div className="stat">
                  <div className="stat-label">success rate</div>
                  <div className="stat-value">{fmtPct(data.success_rate)}</div>
                </div>
                <div className="stat">
                  <div className="stat-label">avg dwell</div>
                  <div className="stat-value">{fmtMs(data.avg_dwell_ms)}</div>
                </div>
                <div className="stat">
                  <div className="stat-label">no-results</div>
                  <div className="stat-value">{data.no_results}</div>
                </div>
                <div className="stat">
                  <div className="stat-label">total events</div>
                  <div className="stat-value">{data.total_events}</div>
                </div>
              </section>

              {data.daily_opens.length > 0 && (
                <section className="analytics-section">
                  <h2>Opens — last {data.daily_opens.length} days</h2>
                  <div className="sparkline" role="img" aria-label="daily opens">
                    {data.daily_opens.map((d) => (
                      <div
                        key={d.day}
                        className="spark-bar"
                        title={`${d.day} — ${d.count} open${d.count === 1 ? "" : "s"}`}
                        style={{
                          height: maxDay > 0 ? `${Math.max(6, (d.count / maxDay) * 100)}%` : "6%",
                        }}
                      />
                    ))}
                  </div>
                </section>
              )}

              {data.top_queries.length > 0 && (
                <section className="analytics-section">
                  <h2>Top queries</h2>
                  <ul className="bar-list">
                    {data.top_queries.map((q) => (
                      <li key={q.q}>
                        <div className="bar-label">
                          <span className="bar-name">{q.q}</span>
                          <span className="bar-count">{q.count}</span>
                        </div>
                        <div className="bar-track">
                          <div
                            className="bar-fill"
                            style={{
                              width: maxQuery > 0 ? `${(q.count / maxQuery) * 100}%` : "0%",
                            }}
                          />
                        </div>
                      </li>
                    ))}
                  </ul>
                </section>
              )}

              {data.top_items.length > 0 && (
                <section className="analytics-section">
                  <h2>Top items</h2>
                  <ul className="bar-list">
                    {data.top_items.map((it) => (
                      <li key={`${it.kind}:${it.name}`}>
                        <div className="bar-label">
                          <span className="bar-name">
                            {it.name}
                            <span className="bar-kind">{fmtKind(it.kind)}</span>
                          </span>
                          <span className="bar-count">{it.count}</span>
                        </div>
                        <div className="bar-track">
                          <div
                            className="bar-fill"
                            style={{
                              width: maxItem > 0 ? `${(it.count / maxItem) * 100}%` : "0%",
                            }}
                          />
                        </div>
                      </li>
                    ))}
                  </ul>
                </section>
              )}

              {data.kind_breakdown.length > 0 && (
                <section className="analytics-section">
                  <h2>By kind</h2>
                  <ul className="bar-list">
                    {data.kind_breakdown.map((k) => (
                      <li key={k.kind}>
                        <div className="bar-label">
                          <span className="bar-name">{fmtKind(k.kind)}</span>
                          <span className="bar-count">{k.count}</span>
                        </div>
                        <div className="bar-track">
                          <div
                            className="bar-fill"
                            style={{
                              width: maxKind > 0 ? `${(k.count / maxKind) * 100}%` : "0%",
                            }}
                          />
                        </div>
                      </li>
                    ))}
                  </ul>
                </section>
              )}

              <section className="analytics-meta">
                <div>
                  <span className="meta-k">first event</span>
                  <span className="meta-v">{fmtTs(data.first_event_ts)}</span>
                </div>
                <div>
                  <span className="meta-k">last event</span>
                  <span className="meta-v">{fmtTs(data.last_event_ts)}</span>
                </div>
                {data.log_path && (
                  <div>
                    <span className="meta-k">log</span>
                    <code className="meta-v">{data.log_path}</code>
                  </div>
                )}
                <div className="analytics-meta-actions">
                  <button type="button" className="btn" onClick={refresh}>
                    Refresh
                  </button>
                  <button type="button" className="btn danger" onClick={clearLog}>
                    Clear log
                  </button>
                </div>
              </section>

              <p className="analytics-footer">
                Local-only. Nothing in this view ever leaves your machine.
              </p>
            </>
          )}
      </div>
    </Screen>
  );
}
