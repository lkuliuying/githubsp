import { useSyncExternalStore } from "react";

/** 界面缩放档位（百分比），整个界面等比放大，只保存在本机。 */
export const ZOOM_LEVELS = [90, 100, 110, 125] as const;
const KEY = "githubsp.zoom";
const listeners = new Set<() => void>();

function read(): number {
  try {
    const saved = Number(localStorage.getItem(KEY));
    return (ZOOM_LEVELS as readonly number[]).includes(saved) ? saved : 100;
  } catch {
    return 100;
  }
}
let current = read();

export function applyZoom() {
  const root = document.documentElement;
  root.style.zoom = current === 100 ? "" : String(current / 100);
  // 视口单位也会被 zoom 放大，外框尺寸要按缩放比例换算回窗口大小。
  root.style.setProperty("--ui-zoom", String(current / 100));
}

export function setZoom(value: number) {
  current = value;
  try {
    localStorage.setItem(KEY, String(value));
  } catch {
    // 存储不可用时只在本次运行中生效。
  }
  applyZoom();
  listeners.forEach((listener) => listener());
}

export const useZoom = () =>
  useSyncExternalStore(
    (listener) => {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
    () => current,
  );
