import { describe, it, expect } from "vitest";
import {
  countdown,
  defaultPolicy,
  mergeEvents,
  taskState,
  terminalText,
  type ExecutionEvent,
  type QueueItem,
} from "../src/core/types";
const item = (state: string) => ({ state }) as QueueItem;
describe("operational issue state", () => {
  it("requires independent success, rather than GitHub closed or provider output", () => {
    expect(taskState()).toBe("TODO");
    expect(taskState(item("RUNNING"))).toBe("IN_PROGRESS");
    expect(taskState(item("VALIDATING"))).toBe("IN_PROGRESS");
    expect(taskState(item("SUCCESS"))).toBe("DONE");
  });
  it("supports quota, retry, failure and blocked lifecycles", () => {
    expect(
      ["QUEUED", "RUNNING", "WAITING_FOR_PROVIDER", "RUNNING", "SUCCESS"].map(
        (s) => taskState(item(s)),
      ),
    ).toEqual(["TODO", "IN_PROGRESS", "WAITING", "IN_PROGRESS", "DONE"]);
    expect(
      ["QUEUED", "RUNNING", "FAILED", "RUNNING", "SUCCESS"].map((s) =>
        taskState(item(s)),
      ),
    ).toEqual(["TODO", "IN_PROGRESS", "FAILED", "IN_PROGRESS", "DONE"]);
    expect(taskState(item("HUMAN_REVIEW"))).toBe("BLOCKED");
  });
});
describe("shared quota clock", () => {
  it("does not invent a reset when none was reported", () => {
    expect(countdown(null, 0)).toBe("Unknown");
    expect(countdown("invalid", 0)).toBe("Unknown");
  });
  it("handles expiry, timezone offsets and long waits", () => {
    expect(
      countdown(
        "2026-10-21T18:37:00+01:00",
        Date.parse("2026-10-21T16:37:00Z"),
      ),
    ).toBe("01:00:00");
    expect(
      countdown("2026-10-21T18:37:00Z", Date.parse("2026-10-22T00:00:00Z")),
    ).toBe("00:00:00");
  });
});
describe("event replay and safe output", () => {
  const e = (sequence: number) => ({ sequence, payload: {} }) as ExecutionEvent;
  it("merges replay and live events without duplication or reordering", () => {
    expect(
      mergeEvents([e(4), e(2)], [e(1), e(4), e(3)]).map((e) => e.sequence),
    ).toEqual([1, 2, 3, 4]);
  });
  it("removes ANSI and OSC control sequences without interpreting HTML", () => {
    expect(terminalText("\u001b[32mPassed\u001b[0m")).toBe("Passed");
    expect(
      terminalText("\u001b]8;;https://evil.test\u0007text\u001b]8;;\u0007"),
    ).toBe("text");
    expect(terminalText("<img onerror=alert(1)>")).toBe(
      "<img onerror=alert(1)>",
    );
  });
  it("defaults to controlled failover and local-only GitHub state", () => {
    expect(defaultPolicy.failover).toBe("ask");
    expect(defaultPolicy.closeIssue).toBe(false);
    expect(defaultPolicy.requireTests).toBe(true);
  });
});
