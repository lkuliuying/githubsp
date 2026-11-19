import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";

const DELAY = 400;
const GAP = 6;
const EDGE = 8;

interface Tip {
  text: string;
  rect: DOMRect;
}

/**
 * 全局悬浮提示：鼠标停在带 `data-tooltip` 的元素上片刻后显示，
 * 键盘聚焦（focus-visible）时立即显示；点击、滚动、移开或按 Esc 时隐藏。
 * 提示渲染在页面最外层，按窗口边缘自动调整位置，不会被列表裁切。
 */
export function TooltipHost() {
  const [tip, setTip] = useState<Tip | null>(null);
  const box = useRef<HTMLDivElement>(null);

  useEffect(() => {
    let timer = 0;
    let current: HTMLElement | null = null;
    const hide = () => {
      window.clearTimeout(timer);
      current = null;
      setTip(null);
    };
    const show = (el: HTMLElement) => {
      const text = el.dataset.tooltip;
      if (!text || !el.isConnected) return;
      setTip({ text, rect: el.getBoundingClientRect() });
    };
    const target = (node: EventTarget | null) =>
      node instanceof Element
        ? node.closest<HTMLElement>("[data-tooltip]")
        : null;
    const over = (event: PointerEvent) => {
      if (event.pointerType === "touch") return;
      const el = target(event.target);
      if (el === current) return;
      hide();
      if (!el) return;
      current = el;
      timer = window.setTimeout(() => current === el && show(el), DELAY);
    };
    const focus = (event: FocusEvent) => {
      const el = target(event.target);
      if (!el || !el.matches(":focus-visible")) return;
      hide();
      current = el;
      show(el);
    };
    const key = (event: KeyboardEvent) => {
      if (event.key === "Escape") hide();
    };
    document.addEventListener("pointerover", over);
    document.addEventListener("pointerdown", hide, true);
    document.addEventListener("focusin", focus);
    document.addEventListener("focusout", hide);
    document.addEventListener("keydown", key);
    document.addEventListener("scroll", hide, true);
    window.addEventListener("blur", hide);
    window.addEventListener("resize", hide);
    return () => {
      window.clearTimeout(timer);
      document.removeEventListener("pointerover", over);
      document.removeEventListener("pointerdown", hide, true);
      document.removeEventListener("focusin", focus);
      document.removeEventListener("focusout", hide);
      document.removeEventListener("keydown", key);
      document.removeEventListener("scroll", hide, true);
      window.removeEventListener("blur", hide);
      window.removeEventListener("resize", hide);
    };
  }, []);

  useLayoutEffect(() => {
    const el = box.current;
    if (!tip || !el) return;
    // 界面缩放时，测量值是缩放后的像素，而 left/top 会再被放大一次，需要换算回来。
    const measured = el.getBoundingClientRect();
    const zoom = el.offsetWidth ? measured.width / el.offsetWidth : 1;
    const { rect } = tip;
    let left = rect.left + rect.width / 2 - measured.width / 2;
    left = Math.max(EDGE, Math.min(left, innerWidth - measured.width - EDGE));
    let top = rect.bottom + GAP;
    if (top + measured.height > innerHeight - EDGE)
      top = rect.top - GAP - measured.height;
    el.style.left = `${left / zoom}px`;
    el.style.top = `${top / zoom}px`;
    el.dataset.ready = "true";
  }, [tip]);

  if (!tip) return null;
  return createPortal(
    <div ref={box} className="tooltip" role="tooltip" key={tip.text}>
      {tip.text}
    </div>,
    document.body,
  );
}
