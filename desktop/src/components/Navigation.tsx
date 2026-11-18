// 改编自 yetone/magpie navTopGo/navTopStep（MIT，Copyright (c) 2026 yetone）。
// 固定来源及许可见 THIRD_PARTY_NOTICES.md；保留上游弹簧参数与逐帧积分方式。
import { useLayoutEffect, useRef } from "react";
import {
  Download,
  Plus,
  LayoutGrid,
  Settings,
} from "lucide-react";
import { navigate, useWorkspace, type View } from "@/lib/api";
import { unread } from "@/lib/model";

const items = [
  { view: "queue", label: "下载队列", icon: Download },
  { view: "new", label: "新建下载", icon: Plus },
  { view: "software", label: "我的软件", icon: LayoutGrid },
  { view: "settings", label: "设置", icon: Settings },
] as const;
const response = 0.42,
  damping = 0.84;

export function Navigation({ reduced }: { reduced: boolean }) {
  const { view, snapshot } = useWorkspace();
  const root = useRef<HTMLElement>(null);
  const thumb = useRef<HTMLDivElement>(null);
  const target = useRef<View>(view);
  target.current = view;
  const motion = useRef({
    y: 0,
    h: 30,
    vy: 0,
    vh: 0,
    at: 0,
    raf: 0,
    seen: false,
  });
  useLayoutEffect(() => {
    const state = motion.current;
    const nav = root.current!,
      highlight = thumb.current!;
    const row = () =>
      nav.querySelector<HTMLElement>(`[data-view="${target.current}"]`)!;
    const paint = () => {
      highlight.style.transform = `translateY(${state.y}px)`;
      highlight.style.height = `${state.h}px`;
    };
    const put = () => {
      state.y = row().offsetTop;
      state.h = row().offsetHeight;
      state.vy = state.vh = 0;
      paint();
    };
    const step = (now: number) => {
      const dt = Math.min(1 / 30, Math.max(0, (now - state.at) / 1000));
      state.at = now;
      const y = row().offsetTop,
        h = row().offsetHeight;
      const k = ((2 * Math.PI) / response) ** 2,
        c = (4 * Math.PI * damping) / response;
      for (let i = 0, d = dt / 4; i < 4; i++) {
        state.vy += (k * (y - state.y) - c * state.vy) * d;
        state.y += state.vy * d;
        state.vh += (k * (h - state.h) - c * state.vh) * d;
        state.h += state.vh * d;
      }
      if (
        Math.abs(y - state.y) < 0.25 &&
        Math.abs(state.vy) < 4 &&
        Math.abs(h - state.h) < 0.25 &&
        Math.abs(state.vh) < 4
      ) {
        put();
        state.raf = 0;
        return;
      }
      paint();
      state.raf = requestAnimationFrame(step);
    };
    if (!state.seen || reduced) {
      put();
      state.seen = true;
    } else {
      state.at = performance.now();
      state.raf = requestAnimationFrame(step);
    }
    const observer = new ResizeObserver(() => {
      if (!state.raf) put();
    });
    observer.observe(nav);
    return () => {
      cancelAnimationFrame(state.raf);
      state.raf = 0;
      observer.disconnect();
    };
  }, [view, reduced]);
  const updates =
    snapshot?.favorites.filter((favorite) => unread(snapshot, favorite))
      .length ?? 0;
  return (
    <aside className="sidebar">
      <nav aria-label="主导航" ref={root}>
        <div className="nav-top-hl" ref={thumb} aria-hidden="true" />
        {items.map(({ view: key, label, icon: Icon }) => (
          <button
            key={key}
            data-view={key}
            aria-current={view === key ? "page" : undefined}
            className={`nav-item ${view === key ? "active" : ""} ${key === "settings" ? "nav-settings" : ""}`}
            onClick={() => navigate(key)}
          >
            <Icon size={16} strokeWidth={1.6} />
            <span>{label}</span>
            {key === "queue" &&
              !!snapshot?.tasks.filter(
                (task) => !["completed", "cancelled"].includes(task.status),
              ).length && (
                <small>
                  {
                    snapshot.tasks.filter(
                      (task) =>
                        !["completed", "cancelled"].includes(task.status),
                    ).length
                  }
                </small>
              )}
            {key === "software" && updates > 0 && <small>{updates}</small>}
          </button>
        ))}
      </nav>
    </aside>
  );
}
