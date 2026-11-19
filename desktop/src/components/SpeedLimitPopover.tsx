// 顶栏的限速入口：点开小弹窗直接改，不用进设置。
// 「轻量弹出表单 + 提交后短暂显示成功」参考 cult-ui popover-form（MIT），见 THIRD_PARTY_NOTICES.md。
import { useEffect, useId, useRef, useState, type FormEvent } from "react";
import { Check, Gauge } from "lucide-react";
import { Button } from "@/components/ui/button";
import { request } from "@/lib/api";
import {
  limitMegabytes,
  parseLimit,
  shortLimit,
  type Settings,
} from "@/lib/model";

const presets = [0, 1, 5, 10, 20];
const showLimit = (kib: number) => String(Number(limitMegabytes(kib).toFixed(3)));

export function SpeedLimitPopover({
  settings,
  disabled,
}: {
  settings: Settings;
  disabled?: boolean;
}) {
  const current = settings.limitKib;
  const [open, setOpen] = useState(false);
  const [text, setText] = useState(showLimit(current));
  const [error, setError] = useState("");
  const [saving, setSaving] = useState(false);
  const [done, setDone] = useState(false);
  const root = useRef<HTMLDivElement>(null);
  const trigger = useRef<HTMLButtonElement>(null);
  const input = useRef<HTMLInputElement>(null);
  const closeTimer = useRef<number>();
  const id = useId();

  const close = (focus = true) => {
    window.clearTimeout(closeTimer.current);
    setOpen(false);
    if (focus) trigger.current?.focus();
  };
  useEffect(() => {
    if (!open) return;
    setText(showLimit(current));
    setError("");
    setDone(false);
    input.current?.select();
    const outside = (event: PointerEvent) => {
      if (!root.current?.contains(event.target as Node)) close(false);
    };
    document.addEventListener("pointerdown", outside);
    return () => document.removeEventListener("pointerdown", outside);
  }, [open]);
  useEffect(() => () => window.clearTimeout(closeTimer.current), []);

  const apply = async (kib: number) => {
    setError("");
    if (kib !== current) {
      setSaving(true);
      try {
        await request("saveSettings", { settings: { ...settings, limitKib: kib } });
      } catch (reason) {
        setError(reason instanceof Error ? reason.message : String(reason));
        return;
      } finally {
        setSaving(false);
      }
    }
    setText(showLimit(kib));
    setDone(true);
    closeTimer.current = window.setTimeout(() => close(), 650);
  };
  const submit = (event: FormEvent) => {
    event.preventDefault();
    if (text.trim() === showLimit(current)) return void apply(current);
    try {
      void apply(parseLimit(text, current));
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
      input.current?.focus();
    }
  };

  return (
    <div
      className="limit-popover"
      ref={root}
      onKeyDown={(event) => {
        if (open && event.key === "Escape") {
          event.preventDefault();
          event.stopPropagation();
          close();
        }
      }}
    >
      <button
        ref={trigger}
        type="button"
        className={`header-limit ${current ? "is-limited" : ""}`}
        aria-haspopup="dialog"
        aria-expanded={open}
        aria-controls={open ? id : undefined}
        title="调整下载限速"
        disabled={disabled}
        onClick={() => (open ? close() : setOpen(true))}
      >
        <Gauge size={13} aria-hidden="true" />
        {current ? `限速 ${shortLimit(current)}` : "不限速"}
      </button>
      {open && (
        <div
          className={`limit-panel ${done ? "is-done" : ""}`}
          id={id}
          role="dialog"
          aria-label="下载限速"
        >
          <form onSubmit={submit}>
            <label htmlFor={`${id}-input`}>下载限速</label>
            <div className="limit-presets" role="group" aria-label="常用限速">
              {presets.map((mb) => {
                const kib = parseLimit(String(mb), current);
                return (
                  <button
                    key={mb}
                    type="button"
                    className={kib === current ? "selected" : ""}
                    aria-pressed={kib === current}
                    disabled={saving}
                    onClick={() => void apply(kib)}
                  >
                    {mb ? `${mb} MB/s` : "不限速"}
                  </button>
                );
              })}
            </div>
            <div className="limit-field">
              <input
                ref={input}
                id={`${id}-input`}
                className="input"
                inputMode="decimal"
                value={text}
                aria-invalid={!!error || undefined}
                aria-describedby={`${id}-hint`}
                onChange={(event) => {
                  setText(event.target.value);
                  setError("");
                }}
              />
              <span className="limit-unit">MB/s</span>
              <Button type="submit" variant="default" size="sm" disabled={saving}>
                应用
              </Button>
            </div>
            {/* 不用 <p>：页面标题栏的样式会隐藏其中的段落。 */}
            <div id={`${id}-hint`} className={error ? "limit-error" : "limit-hint"}>
              {error || "0 表示不限速，立即对正在下载的任务生效"}
            </div>
          </form>
          <div className="limit-success" aria-live="polite">
            {done && (
              <>
                <Check size={16} />
                已应用
              </>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
