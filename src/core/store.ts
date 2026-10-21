import { create } from "zustand";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  mergeEvents,
  boundedEvents,
  providerNames,
  type Snapshot,
  type ExecutionEvent,
} from "./types";

export const desktop = isTauri();
const empty: Snapshot = {
  projects: [],
  issues: [],
  queue: [],
  executions: [],
  providers: [],
  usage: [],
  runningProjects: [],
  githubAccount: null,
};
interface AppStore {
  data: Snapshot;
  projectId: string;
  events: Record<string, ExecutionEvent[]>;
  now: number;
  loading: boolean;
  error: string | null;
  notice: string | null;
  refresh: () => Promise<void>;
  command: <T>(
    command: string,
    args?: Record<string, unknown>,
  ) => Promise<T | undefined>;
  selectProject: (id: string) => void;
  loadEvents: (id: string) => Promise<void>;
  loadEarlier: (id: string) => Promise<void>;
}
export const useApp = create<AppStore>((set, get) => ({
  data: empty,
  projectId: localStorage.getItem("aiflow.project") ?? "",
  events: {},
  now: Date.now(),
  loading: false,
  error: null,
  notice: null,
  refresh: async () => {
    if (!desktop) return;
    try {
      const data = await invoke<Snapshot>("snapshot");
      for (const provider of data.providers)
        providerNames[provider.id] = provider.name;
      set({
        data,
        projectId: data.projects.some((p) => p.id === get().projectId)
          ? get().projectId
          : (data.projects[0]?.id ?? ""),
      });
    } catch (error) {
      set({ error: String(error) });
    }
  },
  command: async <T>(command: string, args?: Record<string, unknown>) => {
    if (!desktop) {
      set({
        error:
          "This operation needs the AiFlow desktop backend. Start with npm run desktop.",
      });
      return undefined;
    }
    set({ loading: true, error: null });
    try {
      const value = await invoke<T>(command, args);
      await get().refresh();
      return value;
    } catch (error) {
      set({ error: String(error) });
      return undefined;
    } finally {
      set({ loading: false });
    }
  },
  selectProject: (projectId) => {
    localStorage.setItem("aiflow.project", projectId);
    set({ projectId });
  },
  loadEvents: async (id) => {
    if (!desktop) return;
    const incoming = await invoke<ExecutionEvent[]>("execution_event_window", {
      executionId: id,
      before: null,
    });
    set((s) => ({
      events: {
        ...s.events,
        [id]: boundedEvents(mergeEvents(s.events[id] ?? [], incoming)),
      },
    }));
  },
  loadEarlier: async (id) => {
    const first = get().events[id]?.find((e) => e.type === "OUTPUT_RECEIVED");
    if (!desktop || !first) return;
    const incoming = await invoke<ExecutionEvent[]>("execution_event_window", {
      executionId: id,
      before: first.sequence,
    });
    // Explicit history paging replaces the output window; complete timeline remains visible.
    set((s) => ({
      events: {
        ...s.events,
        [id]: mergeEvents(
          s.events[id]?.filter((e) => e.type !== "OUTPUT_RECEIVED") ?? [],
          incoming,
        ),
      },
    }));
  },
}));
let refreshTimer: ReturnType<typeof setTimeout> | undefined;
const pending = new Map<string, ExecutionEvent[]>();
let flushTimer: ReturnType<typeof setTimeout> | undefined;
export async function connectEvents() {
  const clock = setInterval(() => useApp.setState({ now: Date.now() }), 1000);
  if (!desktop) return () => clearInterval(clock);
  const output = await listen<ExecutionEvent>("aiflow:event", (event) => {
    const e = event.payload;
    // Unopened consoles replay from durable storage on demand; do not retain every task forever.
    if (!(e.executionId in useApp.getState().events)) return;
    const batch = pending.get(e.executionId) ?? [];
    batch.push(e);
    pending.set(e.executionId, batch);
    if (!flushTimer)
      flushTimer = setTimeout(() => {
        useApp.setState((s) => {
          const events = { ...s.events };
          for (const [id, batch] of pending)
            events[id] = boundedEvents(mergeEvents(events[id] ?? [], batch));
          return { events };
        });
        pending.clear();
        flushTimer = undefined;
      }, 60);
  });
  const changed = await listen("aiflow:changed", () => {
    if (refreshTimer) clearTimeout(refreshTimer);
    refreshTimer = setTimeout(() => void useApp.getState().refresh(), 120);
  });
  await useApp.getState().refresh();
  return () => {
    clearInterval(clock);
    output();
    changed();
    if (flushTimer) clearTimeout(flushTimer);
    if (refreshTimer) clearTimeout(refreshTimer);
  };
}
