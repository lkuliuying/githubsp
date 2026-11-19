import { useCallback, useEffect, useRef, useState } from "react";
import { isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Link, X } from "lucide-react";
import { notify, request } from "@/lib/api";
import {
  describeSuggestion,
  isEditableTarget,
  type ClipboardResult,
  type LinkSuggestion,
} from "@/lib/clipboard";
import { draftSource, openRepository } from "@/views/NewDownload";
import { Button } from "@/components/ui/button";

/** 已经检查过的剪贴板序号；同一份内容只提示一次（跨页面切换保留）。 */
let seen: number | null = null;

function open(suggestion: LinkSuggestion) {
  openRepository(suggestion.source, suggestion.kind === "repository");
}

/**
 * 窗口打开或切回前台时识别剪贴板里的 GitHub 链接，在顶部提示下载；
 * 另外在非输入区域按 Ctrl+V 会直接带着链接打开新建下载。
 */
export function ClipboardPrompt({ enabled }: { enabled: boolean }) {
  const [suggestion, setSuggestion] = useState<LinkSuggestion | null>(null);
  const checking = useRef(false);

  const check = useCallback(async () => {
    if (!enabled || !isTauri() || checking.current) return;
    checking.current = true;
    try {
      const result = await request<ClipboardResult>("clipboard", {
        known: seen,
        manual: false,
      });
      // 序号没变说明内容没变，保留当前横幅状态。
      if (result.sequence === seen) return;
      seen = result.sequence;
      const next = result.suggestion;
      setSuggestion(next && next.source !== draftSource() ? next : null);
    } catch {
      // 剪贴板被其他程序占用等情况，安静跳过。
    } finally {
      checking.current = false;
    }
  }, [enabled]);

  useEffect(() => {
    if (!enabled) {
      setSuggestion(null);
      return;
    }
    void check();
    const onFocus = () => void check();
    const onVisible = () => {
      if (document.visibilityState === "visible") void check();
    };
    window.addEventListener("focus", onFocus);
    document.addEventListener("visibilitychange", onVisible);
    // 托盘点击、再次双击图标时由 Rust 发出，不依赖窗口真正拿到焦点。
    let disposed = false;
    let unlisten: (() => void) | undefined;
    if (isTauri())
      void listen("window-activated", onFocus).then((fn) => {
        if (disposed) fn();
        else unlisten = fn;
      });
    return () => {
      disposed = true;
      unlisten?.();
      window.removeEventListener("focus", onFocus);
      document.removeEventListener("visibilitychange", onVisible);
    };
  }, [enabled, check]);

  useEffect(() => {
    const keydown = (event: KeyboardEvent) => {
      if (
        !(event.ctrlKey || event.metaKey) ||
        event.altKey ||
        event.shiftKey ||
        event.key.toLowerCase() !== "v" ||
        event.isComposing ||
        event.repeat ||
        isEditableTarget(event.target) ||
        document.querySelector('[role="dialog"]') ||
        !isTauri()
      )
        return;
      event.preventDefault();
      void (async () => {
        try {
          const result = await request<ClipboardResult>("clipboard", {
            known: null,
            manual: true,
          });
          seen = result.sequence;
          setSuggestion(null);
          if (result.suggestion) open(result.suggestion);
          else notify("剪贴板里没有识别到 GitHub 仓库、版本页或附件链接");
        } catch (error) {
          notify(String(error), "error");
        }
      })();
    };
    document.addEventListener("keydown", keydown);
    return () => document.removeEventListener("keydown", keydown);
  }, []);

  if (!suggestion) return null;
  const text = describeSuggestion(suggestion);
  return (
    <div className="clipboard-prompt" role="status">
      <span className="clipboard-prompt-icon">
        <Link size={15} />
      </span>
      <div className="clipboard-prompt-text">
        <strong title={text.title}>{text.title}</strong>
        {text.detail && <small>{text.detail}</small>}
      </div>
      <Button
        size="sm"
        variant="default"
        onClick={() => {
          setSuggestion(null);
          open(suggestion);
        }}
      >
        {text.action}
      </Button>
      <Button
        size="icon"
        variant="ghost"
        aria-label="忽略剪贴板链接"
        tooltip="忽略"
        onClick={() => setSuggestion(null)}
      >
        <X size={15} />
      </Button>
    </div>
  );
}
