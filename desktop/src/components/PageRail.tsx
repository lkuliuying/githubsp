import {
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type ReactNode,
  type RefObject,
} from "react";

/** 页面内容宽度达到这个值时，右侧显示常驻侧栏；更窄时保持原来的单栏布局。 */
export const RAIL_MIN_WIDTH = 1320;

/** 观察页面宽度，宽屏时返回 true。 */
export function useWideLayout(ref: RefObject<HTMLElement>) {
  const [wide, setWide] = useState(false);
  useLayoutEffect(() => {
    const element = ref.current;
    if (!element) return;
    const measure = () => {
      // 页面被隐藏时宽度为 0，保留上一次的结果，切回来时不闪动。
      if (element.clientWidth > 0)
        setWide(element.clientWidth >= RAIL_MIN_WIDTH);
    };
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(element);
    return () => observer.disconnect();
  }, [ref]);
  return wide;
}

/** 侧栏里的一张小卡片。 */
export function RailCard({
  title,
  extra,
  children,
  className = "",
}: {
  title: ReactNode;
  extra?: ReactNode;
  children: ReactNode;
  className?: string;
}) {
  return (
    <section className={`rail-card ${className}`}>
      <header className="rail-card-head">
        <h2>{title}</h2>
        {extra}
      </header>
      {children}
    </section>
  );
}

/** 最近一段时间的速度折线，数值单位任意，只看走势。 */
export function Sparkline({ values }: { values: number[] }) {
  const width = 100;
  const height = 32;
  const max = Math.max(...values, 1);
  const step = values.length > 1 ? width / (values.length - 1) : width;
  const points = values.map(
    (value, index) =>
      `${(index * step).toFixed(2)},${(height - 1 - (value / max) * (height - 4)).toFixed(2)}`,
  );
  return (
    <svg
      className="sparkline"
      viewBox={`0 0 ${width} ${height}`}
      preserveAspectRatio="none"
      aria-hidden="true"
    >
      {points.length > 1 && (
        <>
          <polygon
            className="sparkline-area"
            points={`0,${height} ${points.join(" ")} ${width},${height}`}
          />
          <polyline className="sparkline-line" points={points.join(" ")} />
        </>
      )}
    </svg>
  );
}

const SAMPLE_COUNT = 60;
// 放在模块里，切换页面或侧栏重新出现时折线不会从头开始。
const samples: number[] = Array(SAMPLE_COUNT).fill(0);

/** 每秒记录一次总速度，返回最近 60 秒的数值。页面不可见时只记录、不刷新界面。 */
export function useSpeedSamples(speed: number, visible: boolean) {
  const latest = useRef(speed);
  latest.current = speed;
  const [values, setValues] = useState(() => [...samples]);
  const shown = useRef(visible);
  shown.current = visible;
  useEffect(() => {
    const timer = setInterval(() => {
      samples.push(latest.current);
      samples.splice(0, samples.length - SAMPLE_COUNT);
      if (shown.current) setValues([...samples]);
    }, 1000);
    return () => clearInterval(timer);
  }, []);
  useEffect(() => {
    if (visible) setValues([...samples]);
  }, [visible]);
  return values;
}
