// 弹簧参数与逐帧积分改编自 yetone/magpie navTopGo/navTopStep（MIT，Copyright (c) 2026 yetone）；
// 「选中底色在标签间滑动」的交互参考 cult-ui direction-aware-tabs / halo-segmented（MIT）。
// 固定来源及许可见 THIRD_PARTY_NOTICES.md。
import { useLayoutEffect, useRef } from "react";

const response = 0.42,
  damping = 0.84;

type Axis = "x" | "y";

const prefersReduced = () =>
  document.documentElement.classList.contains("reduce-motion") ||
  matchMedia("(prefers-reduced-motion: reduce)").matches;

/**
 * 让一个滑块（thumb）跟随容器里当前选中的子元素，用弹簧动画移动并改变尺寸。
 * - `active` 变化时触发动画；首帧、隐藏页面或减少动态效果时直接就位。
 * - 容器需要 `position: relative`，子元素的 offsetParent 就是容器。
 * - 没有选中项时滑块隐藏。
 */
export function useSpringThumb<R extends HTMLElement = HTMLDivElement>(
  active: string | null | undefined,
  {
    axis = "x",
    selector = ":scope > .selected",
    reduced,
  }: { axis?: Axis; selector?: string; reduced?: boolean } = {},
) {
  const root = useRef<R>(null);
  const thumb = useRef<HTMLSpanElement>(null);
  const state = useRef({
    pos: 0,
    size: 0,
    vp: 0,
    vs: 0,
    at: 0,
    raf: 0,
    seen: false,
    moving: false,
    key: active,
  });
  useLayoutEffect(() => {
    const s = state.current;
    const box = root.current,
      el = thumb.current;
    if (!box || !el) return;
    const target = () => box.querySelector<HTMLElement>(selector);
    const measure = (row: HTMLElement) =>
      axis === "x"
        ? [row.offsetLeft, row.offsetWidth]
        : [row.offsetTop, row.offsetHeight];
    const paint = () => {
      el.style.transform =
        axis === "x" ? `translateX(${s.pos}px)` : `translateY(${s.pos}px)`;
      el.style[axis === "x" ? "width" : "height"] = `${s.size}px`;
    };
    const put = () => {
      const row = target();
      // 隐藏页面里尺寸是 0，等可见后由 ResizeObserver 再就位。
      if (!row || !row.offsetWidth) {
        el.style.opacity = "0";
        s.seen = false;
        return;
      }
      [s.pos, s.size] = measure(row);
      s.vp = s.vs = 0;
      el.style.opacity = "1";
      s.seen = true;
      paint();
    };
    const step = (now: number) => {
      const row = target();
      if (!row) {
        s.raf = 0;
        s.moving = false;
        return put();
      }
      const dt = Math.min(1 / 30, Math.max(0, (now - s.at) / 1000));
      s.at = now;
      const [pos, size] = measure(row);
      const k = ((2 * Math.PI) / response) ** 2,
        c = (4 * Math.PI * damping) / response;
      for (let i = 0, d = dt / 4; i < 4; i++) {
        s.vp += (k * (pos - s.pos) - c * s.vp) * d;
        s.pos += s.vp * d;
        s.vs += (k * (size - s.size) - c * s.vs) * d;
        s.size += s.vs * d;
      }
      if (
        Math.abs(pos - s.pos) < 0.25 &&
        Math.abs(s.vp) < 4 &&
        Math.abs(size - s.size) < 0.25 &&
        Math.abs(s.vs) < 4
      ) {
        s.raf = 0;
        s.moving = false;
        put();
        return;
      }
      paint();
      s.raf = requestAnimationFrame(step);
    };
    // 每次渲染都会走到这里（快照每秒刷新）：只有选中项变了、或上一段动画还没走完才继续弹簧，
    // 其余情况直接就位，避免空转动画帧。
    const changed = s.key !== active;
    s.key = active;
    if (!s.seen || (reduced ?? prefersReduced())) {
      s.moving = false;
      put();
    } else if (changed || s.moving) {
      s.moving = true;
      s.at = performance.now();
      s.raf = requestAnimationFrame(step);
    } else put();
    // 计数角标出现或消失会改变按钮宽度，所以同时观察容器和每个子元素。
    const observer = new ResizeObserver(() => {
      if (!s.moving) put();
    });
    observer.observe(box);
    for (const child of Array.from(box.children)) observer.observe(child);
    return () => {
      cancelAnimationFrame(s.raf);
      s.raf = 0;
      observer.disconnect();
    };
  });
  // 不传依赖：每次渲染都重新挂观察器，子元素增减（比如「异常」筛选出现）也能跟上。
  return { root, thumb };
}
