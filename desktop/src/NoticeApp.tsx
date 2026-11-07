import { useEffect, useState } from "react";
import { ArrowDownToLine, ArrowUpRight, Bell, X } from "lucide-react";
import { connect, notify, request, useWorkspace } from "./lib/api";
import { Button } from "./components/ui/button";
import { useJob } from "./components/common";

export function NoticeApp() {
  const { snapshot, message } = useWorkspace();
  const { run } = useJob();
  const [paused, setPaused] = useState(false);
  useEffect(() => {
    const controller = new AbortController();
    void connect(controller.signal);
    return () => controller.abort();
  }, []);
  useEffect(() => {
    if (paused || !snapshot) return;
    const timer = setTimeout(() => {
      void request("hideNotice").catch((error) =>
        notify(String(error), "error"),
      );
    }, 8000);
    return () => clearTimeout(timer);
  }, [paused, snapshot?.notices.map((n) => n.id).join(",")]);
  const notices = [...(snapshot?.notices ?? [])]
    .sort((a, b) => b.createdAt - a.createdAt)
    .slice(0, 4);
  return (
    <div
      className="desktop-notice"
      onMouseEnter={() => setPaused(true)}
      onMouseLeave={() => setPaused(false)}
      onFocusCapture={() => setPaused(true)}
      onBlurCapture={(event) => {
        if (!event.currentTarget.contains(event.relatedTarget))
          setPaused(false);
      }}
    >
      <header data-tauri-drag-region>
        <span>
          <Bell size={15} />
          GitHubSP
        </span>
        <Button
          size="icon"
          variant="ghost"
          aria-label="收起提醒"
          onClick={() => run(() => request("hideNotice"))}
        >
          <X size={16} />
        </Button>
      </header>
      <h1>下载与项目更新</h1>
      <div className="desktop-notice-list">
        {notices.map((notice) => (
          <button
            key={notice.id}
            onClick={() =>
              run(async () => {
                await request("showMain", {
                  view:
                    notice.kind === "favorite_updated" ? "favorites" : "queue",
                });
                await request("hideNotice");
              })
            }
          >
            <ArrowDownToLine size={16} />
            <span>{notice.message}</span>
            <ArrowUpRight size={14} />
          </button>
        ))}
      </div>
      {message?.kind === "error" && (
        <p className="danger-text">{message.text}</p>
      )}
      <footer>悬停或键盘操作时保留提醒</footer>
    </div>
  );
}
