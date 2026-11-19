import type { CSSProperties } from "react";

// 数量变化时数字上下滚动到新值，参考 cult-ui rolling-number（MIT），见 THIRD_PARTY_NOTICES.md。
// 这里不用 motion 的弹簧逐值计数，而是每一位一个 0–9 的竖条，用 CSS 过渡移到目标数字：
// 计数只在整数间跳，竖条更直观，也不需要额外依赖。数字竖条画在伪元素里，
// 文本内容仍是真实数值，读屏和脚本读到的都是正常数字。
export function RollingNumber({ value }: { value: number }) {
  const digits = String(Math.max(0, Math.round(value))).split("");
  return (
    <span className="rolling-number">
      <span className="sr-only">{value}</span>
      <span className="rolling-digits" aria-hidden="true">
        {digits.map((digit, index) => (
          // 从个位往左编号，位数增加时原有各位保持同一个元素，继续滚动。
          <span
            key={digits.length - index}
            className="rolling-digit"
            style={{ "--digit": digit } as CSSProperties}
          />
        ))}
      </span>
    </span>
  );
}
