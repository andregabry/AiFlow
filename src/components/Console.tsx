import { useEffect, useRef, useState, useMemo } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import {
  ArrowDownToLine,
  Copy,
  Search,
  Maximize2,
  Trash2,
  History,
} from "lucide-react";
import { useApp } from "../core/store";
import { terminalText, type ExecutionEvent } from "../core/types";
import { AiButton, AiSegmentedControl } from "./ui";

export function AiExecutionConsole({
  executionId,
  wrap = false,
  label = "LIVE AGENT OUTPUT",
}: {
  executionId: string;
  wrap?: boolean;
  label?: string;
}) {
  const events = useApp((s) => s.events[executionId]);
  const [mode, setMode] = useState<"Formatted" | "Raw">("Formatted");
  const [search, setSearch] = useState("");
  const [follow, setFollow] = useState(true);
  const [clearedAt, setClearedAt] = useState(0);
  const [expanded, setExpanded] = useState(false);
  const parent = useRef<HTMLDivElement>(null);
  useEffect(() => {
    void useApp
      .getState()
      .loadEvents(executionId)
      .catch((e) => useApp.setState({ error: String(e) }));
  }, [executionId]);
  const rows = useMemo(
    () =>
      (events ?? [])
        .filter((e) => e.type === "OUTPUT_RECEIVED" && e.sequence > clearedAt)
        .flatMap((e) => {
          const raw = e.payload.content ?? "";
          const text =
            mode === "Raw"
              ? raw
              : (e.payload.formatted?.display ??
                e.payload.formatted?.text ??
                raw);
          return terminalText(text)
            .split("\n")
            .filter(Boolean)
            .map((line, i) => ({
              event: e,
              text: line,
              key: `${e.sequence}-${i}`,
            }));
        })
        .filter((row) => row.text.toLowerCase().includes(search.toLowerCase())),
    [events, mode, search, clearedAt],
  );
  // Variable content wraps visually only within a capped row; raw is available for copying.
  // eslint-disable-next-line react-hooks/incompatible-library
  const virtual = useVirtualizer({
    count: rows.length,
    getScrollElement: () => parent.current,
    estimateSize: () => (wrap ? 58 : 29),
    overscan: 15,
  });
  useEffect(() => {
    if (follow && rows.length)
      virtual.scrollToIndex(rows.length - 1, { align: "end" });
  }, [rows.length, follow, virtual]);
  async function earlier() {
    setFollow(false);
    setClearedAt(0);
    try {
      await useApp.getState().loadEarlier(executionId);
    } catch (e) {
      useApp.setState({ error: String(e) });
    }
  }
  async function copy() {
    try {
      await navigator.clipboard.writeText(rows.map((r) => r.text).join("\n"));
      useApp.setState({ notice: "Visible output copied" });
    } catch (e) {
      useApp.setState({ error: String(e) });
    }
  }
  return (
    <section
      className={`console ${expanded ? "console-expanded" : ""} ${wrap ? "console-wrap" : ""}`}
      aria-label={label}
    >
      <div className="console-toolbar">
        <div className="live-label">
          <span className="live-dot" /> {label}
        </div>
        <AiSegmentedControl
          value={mode}
          options={["Formatted", "Raw"]}
          onChange={setMode}
        />
        <div className="console-actions">
          <AiButton
            variant="ghost"
            aria-label="Load earlier output"
            onClick={() => void earlier()}
          >
            <History size={14} />
          </AiButton>
          <AiButton
            variant="ghost"
            aria-label="Copy output"
            onClick={() => void copy()}
          >
            <Copy size={14} />
          </AiButton>
          <AiButton
            variant="ghost"
            aria-label="Clear view, preserve logs"
            onClick={() => setClearedAt(events?.at(-1)?.sequence ?? 0)}
          >
            <Trash2 size={14} />
          </AiButton>
          <AiButton
            variant="ghost"
            aria-label="Maximize output"
            onClick={() => setExpanded((v) => !v)}
          >
            <Maximize2 size={14} />
          </AiButton>
        </div>
      </div>
      <div className="console-search">
        <Search size={13} />
        <input
          placeholder="Find in output…"
          aria-label="Search logs"
          value={search}
          onChange={(e) => setSearch(e.target.value)}
        />
        <button
          className={follow ? "selected-text" : ""}
          onClick={() => {
            if (!follow) {
              setClearedAt(0);
              void useApp
                .getState()
                .loadEvents(executionId)
                .catch((e) => useApp.setState({ error: String(e) }));
            }
            setFollow((v) => !v);
          }}
        >
          <ArrowDownToLine size={12} /> Follow
        </button>
        <span>{rows.length.toLocaleString()} lines</span>
      </div>
      <div
        ref={parent}
        className="console-scroll"
        tabIndex={0}
        onWheel={() => setFollow(false)}
      >
        {rows.length ? (
          <div style={{ height: virtual.getTotalSize(), position: "relative" }}>
            {virtual.getVirtualItems().map((v) => {
              const row = rows[v.index];
              return (
                <div
                  key={row.key}
                  data-index={v.index}
                  ref={wrap ? virtual.measureElement : undefined}
                  className={`log-line ${row.event.payload.stream === "stderr" ? "stderr" : ""}`}
                  style={{
                    position: "absolute",
                    top: 0,
                    left: 0,
                    width: "100%",
                    transform: `translateY(${v.start}px)`,
                    height: wrap ? undefined : 29,
                  }}
                >
                  <time>
                    {new Date(row.event.timestamp).toLocaleTimeString()}
                  </time>
                  <pre title={row.text}>{row.text}</pre>
                </div>
              );
            })}
          </div>
        ) : (
          <div className="console-empty">
            <span>⌁</span>
            <p>Output will appear here as the agent works.</p>
            <small>
              stdout + stderr · persisted locally · secrets redacted
            </small>
          </div>
        )}
      </div>
    </section>
  );
}
export function AiTimeline({ events }: { events: ExecutionEvent[] }) {
  return (
    <div className="timeline">
      {events
        .filter((e) => e.type !== "OUTPUT_RECEIVED")
        .map((e) => (
          <div key={e.sequence}>
            <span className="timeline-dot" />
            <time>{new Date(e.timestamp).toLocaleTimeString()}</time>
            <strong>{e.type.replaceAll("_", " ").toLowerCase()}</strong>
            {e.payload.state && <small>{e.payload.state}</small>}
            {e.payload.reason && <p>{e.payload.reason}</p>}
          </div>
        ))}
    </div>
  );
}
