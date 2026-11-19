// 「复制后图标换成对勾，稍后复原」的反馈参考 cult-ui copy-button（MIT），见 THIRD_PARTY_NOTICES.md。
// 这里不用 motion 库，图标切换用 CSS 过渡实现，跟随项目的减少动态效果设置。
import { useEffect, useRef, useState } from "react";
import { Check, Copy } from "lucide-react";
import { Button, type ButtonProps } from "@/components/ui/button";
import { notify, request } from "@/lib/api";
import { cn } from "@/lib/utils";

const resetAfter = 1600;

export interface CopyButtonProps extends Omit<ButtonProps, "onClick" | "children"> {
  text: string | null | undefined;
  /** 有文字时显示为普通按钮；不传则只显示图标，需要同时传 aria-label。 */
  label?: string;
  copiedLabel?: string;
}

export function CopyButton({
  text,
  label,
  copiedLabel = "已复制",
  className,
  disabled,
  ...props
}: CopyButtonProps) {
  const [copied, setCopied] = useState(false);
  const timer = useRef<number>();
  useEffect(() => () => window.clearTimeout(timer.current), []);
  const copy = async () => {
    if (!text) return;
    try {
      await request("copy", { text });
      setCopied(true);
      window.clearTimeout(timer.current);
      timer.current = window.setTimeout(() => setCopied(false), resetAfter);
    } catch (error) {
      notify(String(error instanceof Error ? error.message : error), "error");
    }
  };
  return (
    <Button
      {...props}
      disabled={disabled || !text}
      className={cn("copy-button", copied && "is-copied", className)}
      onClick={copy}
    >
      <span className="copy-icon" aria-hidden="true">
        <Copy size={14} />
        <Check size={14} />
      </span>
      {label && (
        // 两段文字叠在同一格里，切换时按钮宽度不跳。
        <span className="copy-label">
          <span aria-hidden={copied}>{label}</span>
          <span aria-hidden={!copied}>{copiedLabel}</span>
        </span>
      )}
      <span className="sr-only" aria-live="polite">
        {copied ? copiedLabel : ""}
      </span>
    </Button>
  );
}
