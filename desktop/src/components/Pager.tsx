import { useState, type ReactNode } from "react";
import { ChevronLeft, ChevronRight } from "lucide-react";
import { Button } from "./ui/button";

const SIZES = [10, 20, 50, 100];

/** 每个列表各自记住「每页条数」，只保存在本机浏览器存储里。 */
export function usePageSize(key: string, initial = 20) {
  const storageKey = `githubsp.pageSize.${key}`;
  const [size, setSize] = useState(() => {
    try {
      const saved = Number(localStorage.getItem(storageKey));
      return SIZES.includes(saved) ? saved : initial;
    } catch {
      return initial;
    }
  });
  const change = (value: number) => {
    setSize(value);
    try {
      localStorage.setItem(storageKey, String(value));
    } catch {
      // 存储不可用时只在本次运行中生效。
    }
  };
  return [size, change] as const;
}

/** 把页码限制在有效范围内，列表变短时不会停在空白页。 */
export function clampPage(page: number, total: number, size: number) {
  return Math.min(Math.max(1, page), Math.max(1, Math.ceil(total / size)));
}

export function Pager({
  page,
  total,
  pageSize,
  onPage,
  onPageSize,
  disabled = false,
  unit = "条",
  extra,
}: {
  page: number;
  total: number;
  pageSize: number;
  onPage(page: number): void;
  onPageSize(size: number): void;
  disabled?: boolean;
  unit?: string;
  extra?: ReactNode;
}) {
  const pages = Math.max(1, Math.ceil(total / pageSize));
  return (
    <footer className="list-footer pager">
      <span className="pager-total">
        共 {total} {unit}
      </span>
      {total > SIZES[0] && (
        <label className="pager-size">
          每页
          <select
            aria-label="每页显示条数"
            value={pageSize}
            onChange={(event) => {
              onPageSize(Number(event.target.value));
              onPage(1);
            }}
          >
            {SIZES.map((size) => (
              <option key={size} value={size}>
                {size}
              </option>
            ))}
          </select>
          {unit}
        </label>
      )}
      <span className="pager-spacer" />
      {extra}
      {pages > 1 && (
        <div className="list-pages">
          <Button
            size="icon"
            variant="ghost"
            aria-label="上一页"
            disabled={disabled || page <= 1}
            onClick={() => onPage(page - 1)}
          >
            <ChevronLeft size={16} />
          </Button>
          <span>
            {page} / {pages}
          </span>
          <Button
            size="icon"
            variant="ghost"
            aria-label="下一页"
            disabled={disabled || page >= pages}
            onClick={() => onPage(page + 1)}
          >
            <ChevronRight size={16} />
          </Button>
        </div>
      )}
    </footer>
  );
}
