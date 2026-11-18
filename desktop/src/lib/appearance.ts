import { useSyncExternalStore } from "react";

/** 外观：浅色 / 深色 / 跟随系统，以及主题色。只保存在本机。 */
export type ThemeMode = "light" | "dark" | "system";
export const ACCENTS = [
  ["blue", "蓝色"],
  ["teal", "青绿"],
  ["purple", "紫色"],
  ["graphite", "石墨"],
] as const;
export type Accent = (typeof ACCENTS)[number][0];

const KEY = "githubsp.appearance";
const LEGACY_KEY = "githubsp.theme";
const listeners = new Set<() => void>();
const media = matchMedia("(prefers-color-scheme: dark)");

interface Appearance {
  mode: ThemeMode;
  accent: Accent;
  dark: boolean;
}

function read(): Omit<Appearance, "dark"> {
  try {
    const saved = JSON.parse(localStorage.getItem(KEY) ?? "null");
    if (saved && ["light", "dark", "system"].includes(saved.mode))
      return {
        mode: saved.mode,
        accent: ACCENTS.some(([key]) => key === saved.accent) ? saved.accent : "blue",
      };
    // 旧版只保存了浅色/深色。
    const legacy = localStorage.getItem(LEGACY_KEY);
    return { mode: legacy === "dark" ? "dark" : "light", accent: "blue" };
  } catch {
    return { mode: "light", accent: "blue" };
  }
}

const resolve = (mode: ThemeMode) => mode === "dark" || (mode === "system" && media.matches);
let state: Appearance = (() => {
  const saved = read();
  return { ...saved, dark: resolve(saved.mode) };
})();

export function applyAppearance() {
  const root = document.documentElement;
  root.classList.toggle("dark", state.dark);
  root.dataset.accent = state.accent;
}

function update(next: Partial<Omit<Appearance, "dark">>) {
  const mode = next.mode ?? state.mode;
  state = { mode, accent: next.accent ?? state.accent, dark: resolve(mode) };
  try {
    localStorage.setItem(KEY, JSON.stringify({ mode: state.mode, accent: state.accent }));
  } catch {
    // 存储不可用时只在本次运行中生效。
  }
  applyAppearance();
  listeners.forEach((listener) => listener());
}

export const setThemeMode = (mode: ThemeMode) => update({ mode });
export const setAccent = (accent: Accent) => update({ accent });

media.addEventListener("change", () => {
  if (state.mode === "system") update({});
});
// 另一个窗口（主窗口 / 提醒窗口）修改外观时同步过来。
window.addEventListener("storage", (event) => {
  if (event.key !== KEY && event.key !== LEGACY_KEY) return;
  const saved = read();
  state = { ...saved, dark: resolve(saved.mode) };
  applyAppearance();
  listeners.forEach((listener) => listener());
});

export const useAppearance = () =>
  useSyncExternalStore(
    (listener) => {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
    () => state,
  );
