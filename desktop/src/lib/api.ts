import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useSyncExternalStore } from "react";
import type { Bootstrap, Snapshot } from "./model";

export type View =
  | "queue"
  | "new"
  | "software"
  | "settings";
type Message = { id: number; text: string; kind: "error" | "success" | "info" };
interface State {
  info: Bootstrap | null;
  snapshot: Snapshot | null;
  error: string | null;
  message: Message | null;
  view: View;
  connected: boolean;
}
let state: State = {
  info: null,
  snapshot: null,
  error: null,
  message: null,
  view: "queue",
  connected: false,
};
const listeners = new Set<() => void>();
function update(next: Partial<State>) {
  state = { ...state, ...next };
  listeners.forEach((fn) => fn());
}
export const useWorkspace = () =>
  useSyncExternalStore(
    (fn) => {
      listeners.add(fn);
      return () => listeners.delete(fn);
    },
    () => state,
  );
const navigationBlocks = new Set<symbol>();
let pendingView: View | null = null;
export function navigate(view: View) {
  if (navigationBlocks.size) {
    pendingView = view;
    return;
  }
  if (state.view !== view) update({ view });
}
export function blockNavigation() {
  const token = Symbol();
  navigationBlocks.add(token);
  return () => {
    navigationBlocks.delete(token);
    if (!navigationBlocks.size && pendingView) {
      const next = pendingView;
      pendingView = null;
      navigate(next);
    }
  };
}
export const notify = (text: string, kind: Message["kind"] = "info") =>
  update({ message: { id: Date.now(), text, kind } });
export const dismissMessage = () => update({ message: null });
export function acceptSnapshot(snapshot: Snapshot) {
  // 初始请求和推送可能交错，旧修订不能覆盖较新的下载状态。
  if (!state.snapshot || snapshot.revision >= state.snapshot.revision)
    update({ snapshot });
}
export async function request<T = unknown>(
  type: string,
  args: Record<string, unknown> = {},
): Promise<T> {
  if (!isTauri())
    throw new Error("这是浏览器预览。请运行桌面版以访问本地任务。");
  const value = await invoke<T>("dispatch", { request: { type, ...args } });
  const snapshot = value as Partial<Snapshot> | null;
  if (
    snapshot &&
    typeof snapshot.revision === "number" &&
    Array.isArray(snapshot.tasks)
  )
    acceptSnapshot(snapshot as Snapshot);
  const nested = (value as { snapshot?: Snapshot } | null)?.snapshot;
  if (
    nested &&
    typeof nested.revision === "number" &&
    Array.isArray(nested.tasks)
  )
    acceptSnapshot(nested);
  return value;
}
export async function connect(signal: AbortSignal) {
  if (!isTauri()) {
    update({
      error:
        "浏览器预览 · 本地数据尚未连接。通过 pnpm desktop 启动完整桌面版。",
    });
    return () => {};
  }
  const disposers: (() => void)[] = [];
  const dispose = () => disposers.splice(0).forEach((fn) => fn());
  signal.addEventListener("abort", dispose, { once: true });
  const register = (fn: () => void) => {
    if (signal.aborted) fn();
    else disposers.push(fn);
  };
  try {
    register(
      await listen<Snapshot>("workspace-state", (event) =>
        acceptSnapshot(event.payload),
      ),
    );
    register(
      await listen<string>("workspace-error", (event) =>
        notify(event.payload, "error"),
      ),
    );
    register(
      await listen<View>("workspace-navigate", (event) =>
        navigate(event.payload),
      ),
    );
    if (signal.aborted) return dispose;
    const info = await request<Bootstrap>("bootstrap");
    if (signal.aborted) return dispose;
    acceptSnapshot(info.snapshot);
    update({ info, connected: true, error: null });
  } catch (error) {
    dispose();
    if (!signal.aborted) update({ error: String(error), connected: false });
  }
  return () => {
    signal.removeEventListener("abort", dispose);
    dispose();
  };
}
