// 拖动排序：按住把手上下拖，其他行让位，松手后提交新位置。
// 交互参考 cult-ui sortable-list（MIT），见 THIRD_PARTY_NOTICES.md。
// 用指针事件实现，不用 HTML5 拖放：Tauri 在 Windows 上接管了窗口的文件拖放，HTML5 拖放事件不可靠。
import {
  useLayoutEffect,
  useRef,
  useState,
  type KeyboardEvent,
  type PointerEvent,
} from "react";

interface Drag {
  id: string;
  from: number;
  to: number;
  startY: number;
  rows: HTMLElement[];
  mids: number[];
  step: number;
  min: number;
  max: number;
  handle: HTMLElement;
  pointer: number;
  escape: (event: globalThis.KeyboardEvent) => void;
}

/**
 * - `ids`：可以拖动的行，按当前显示顺序，必须在页面上连续排列。
 * - `onMove(id, to)`：松手后调用，`to` 是在 `ids` 里的新位置。调用方需要在同一次渲染里换成新顺序，
 *   这样清掉位移时行已经在新位置上，不会闪。
 * - `onStep(id, direction)`：把手获得焦点时用方向键移动一位。
 */
export function useDragSort({
  ids,
  disabled,
  onMove,
  onStep,
}: {
  ids: string[];
  disabled: boolean;
  onMove(id: string, to: number): void;
  onStep(id: string, direction: "up" | "down" | "top"): void;
}) {
  const rows = useRef(new Map<string, HTMLElement>());
  const drag = useRef<Drag | null>(null);
  const settle = useRef<HTMLElement[] | null>(null);
  const [dragging, setDragging] = useState<string | null>(null);

  // 新顺序渲染完成、浏览器绘制前，不带过渡地清掉位移：此时行已经在新位置上。
  useLayoutEffect(() => {
    const list = settle.current;
    if (!list) return;
    settle.current = null;
    for (const row of list) {
      row.style.transition = "none";
      row.style.transform = "";
    }
    void list[0]?.offsetHeight;
    for (const row of list) row.style.transition = "";
  });

  const finish = (commit: boolean) => {
    const state = drag.current;
    if (!state) return;
    drag.current = null;
    window.removeEventListener("keydown", state.escape, true);
    if (state.handle.hasPointerCapture(state.pointer))
      state.handle.releasePointerCapture(state.pointer);
    setDragging(null);
    if (commit && state.to !== state.from) {
      settle.current = state.rows;
      onMove(state.id, state.to);
    } else {
      // 没有移动或取消：行带过渡回到原位。
      state.rows.forEach((row) => (row.style.transform = ""));
    }
  };

  const handle = (id: string) => ({
    onPointerDown(event: PointerEvent<HTMLElement>) {
      if (disabled || event.button !== 0 || drag.current) return;
      const list = ids.map((key) => rows.current.get(key));
      if (list.some((row) => !row)) return;
      const elements = list as HTMLElement[];
      const rects = elements.map((row) => row.getBoundingClientRect());
      const from = ids.indexOf(id);
      if (from < 0) return;
      event.preventDefault();
      event.currentTarget.setPointerCapture(event.pointerId);
      const step =
        rects.length > 1 ? rects[1].top - rects[0].top : rects[0].height;
      const escape = (key: globalThis.KeyboardEvent) => {
        if (key.key !== "Escape") return;
        key.preventDefault();
        key.stopPropagation();
        finish(false);
      };
      window.addEventListener("keydown", escape, true);
      drag.current = {
        id,
        from,
        to: from,
        startY: event.clientY,
        rows: elements,
        escape,
        mids: rects.map((rect) => rect.top + rect.height / 2),
        step,
        min: rects[0].top - rects[from].top,
        max: rects[rects.length - 1].top - rects[from].top,
        handle: event.currentTarget,
        pointer: event.pointerId,
      };
      setDragging(id);
    },
    onPointerMove(event: PointerEvent<HTMLElement>) {
      const state = drag.current;
      if (!state || event.pointerId !== state.pointer) return;
      const dy = Math.min(
        state.max,
        Math.max(state.min, event.clientY - state.startY),
      );
      const center = state.mids[state.from] + dy;
      let to = state.from;
      state.mids.forEach((mid, index) => {
        // 位移被限制在首尾行之间，拖到底时中心正好落在末行中线上，所以用 <= / >=。
        if (index < state.from && center <= mid) to = Math.min(to, index);
        if (index > state.from && center >= mid) to = Math.max(to, index);
      });
      state.to = to;
      state.rows.forEach((row, index) => {
        if (index === state.from) {
          row.style.transform = `translateY(${dy}px)`;
          return;
        }
        const shift =
          state.from < index && index <= to
            ? -state.step
            : to <= index && index < state.from
              ? state.step
              : 0;
        row.style.transform = shift ? `translateY(${shift}px)` : "";
      });
    },
    onPointerUp(event: PointerEvent<HTMLElement>) {
      if (drag.current?.pointer === event.pointerId) finish(true);
    },
    onPointerCancel(event: PointerEvent<HTMLElement>) {
      if (drag.current?.pointer === event.pointerId) finish(false);
    },
    onLostPointerCapture(event: PointerEvent<HTMLElement>) {
      if (drag.current?.pointer === event.pointerId) finish(true);
    },
    onKeyDown(event: KeyboardEvent<HTMLElement>) {
      if (drag.current || disabled) return;
      const index = ids.indexOf(id);
      const direction =
        event.key === "ArrowUp" && index > 0
          ? "up"
          : event.key === "ArrowDown" && index < ids.length - 1
            ? "down"
            : event.key === "Home" && index > 0
              ? "top"
              : null;
      if (!direction) return;
      event.preventDefault();
      onStep(id, direction);
    },
  });

  const register = (id: string) => (row: HTMLElement | null) => {
    if (row) rows.current.set(id, row);
    else rows.current.delete(id);
  };

  // 组件卸载时不留下全局监听。
  useLayoutEffect(
    () => () => {
      if (drag.current)
        window.removeEventListener("keydown", drag.current.escape, true);
    },
    [],
  );
  return { handle, register, dragging };
}
