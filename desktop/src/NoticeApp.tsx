import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { isTauri } from "@tauri-apps/api/core";
import {
  getCurrentWindow,
  LogicalSize,
  PhysicalPosition,
  primaryMonitor,
} from "@tauri-apps/api/window";
import {
  AlertCircle,
  ArrowUpRight,
  BellRing,
  CheckCircle2,
  FolderOpen,
  X,
} from "lucide-react";
import { connect, notify, request, useWorkspace } from "./lib/api";
import { Button } from "./components/ui/button";
import { useJob } from "./components/common";
import { bytes, duration, relativeTime, type Snapshot } from "./lib/model";
import appIcon from "./assets/app-icon.svg";

type Notice = Snapshot["notices"][number];
/** 提醒在屏幕上停留的时间；鼠标悬停或键盘操作时暂停。 */
const VISIBLE_MS = 8000;

/** 把一条提醒整理成标题、主要内容和补充信息。 */
function describe(notice: Notice, snapshot: Snapshot) {
  const task = notice.taskId
    ? snapshot.tasks.find((item) => item.id === notice.taskId)
    : undefined;
  if (notice.kind === "favorite_updated") {
    // 后端文案为「项目有新版本：owner/repo · tag」。
    const [repository, tag] = notice.message.replace(/^.*?：/, "").split(" · ");
    return {
      tone: "update" as const,
      title: "发现新版本",
      name: repository || notice.message,
      meta: tag ? [tag] : [],
      task,
    };
  }
  const failed = notice.kind === "download_failed";
  const elapsed = task?.elapsedMs != null ? duration(task.elapsedMs / 1000) : null;
  return {
    tone: failed ? ("failed" as const) : ("done" as const),
    title: failed ? "下载失败" : "下载完成",
    name: task?.filename ?? notice.message,
    meta: [
      task?.repository,
      task?.tag,
      task && !failed ? bytes(task.total ?? task.downloaded) : null,
      elapsed && !failed ? `用时 ${elapsed}` : null,
    ].filter((value): value is string => !!value),
    task,
  };
}

export function NoticeApp() {
  const { snapshot, message } = useWorkspace();
  const { busy, run } = useJob();
  const [paused, setPaused] = useState(false);
  useEffect(() => {
    const controller = new AbortController();
    void connect(controller.signal);
    return () => controller.abort();
  }, []);
  const notices = [...(snapshot?.notices ?? [])].sort((a, b) => b.createdAt - a.createdAt);
  const key = notices.map((notice) => notice.id).join(",");
  useEffect(() => {
    if (paused || !snapshot) return;
    const timer = setTimeout(() => {
      void request("hideNotice").catch((error) => notify(String(error), "error"));
    }, VISIBLE_MS);
    return () => clearTimeout(timer);
  }, [paused, key]);
  // 窗口高度跟随内容，并贴住屏幕右下角（与 Rust 创建窗口时的位置一致）。
  const root = useRef<HTMLDivElement>(null);
  useLayoutEffect(() => {
    if (!isTauri() || !root.current) return;
    const height = Math.ceil(root.current.getBoundingClientRect().height);
    if (height < 60) return;
    void (async () => {
      try {
        const win = getCurrentWindow();
        await win.setSize(new LogicalSize(380, height));
        const monitor = await primaryMonitor();
        if (!monitor) return;
        const scale = monitor.scaleFactor;
        await win.setPosition(
          new PhysicalPosition(
            Math.max(monitor.position.x, monitor.position.x + monitor.size.width - Math.round(400 * scale)),
            Math.max(monitor.position.y, monitor.position.y + monitor.size.height - Math.round((height + 75) * scale)),
          ),
        );
      } catch {
        // 无法调整尺寸时保留默认窗口大小。
      }
    })();
  }, [key, message?.text, !!snapshot]);
  const hide = () => run(() => request("hideNotice"));
  const open = (notice: Notice) =>
    run(async () => {
      await request("showMain", {
        view: notice.kind === "favorite_updated" ? "software" : "queue",
      });
      await request("hideNotice");
    });
  if (!snapshot || !notices.length)
    return <div className="desktop-notice" data-tauri-drag-region />;
  const [latest, ...rest] = notices;
  const info = describe(latest, snapshot);
  const Icon =
    info.tone === "done" ? CheckCircle2 : info.tone === "failed" ? AlertCircle : BellRing;
  return (
    <div
      ref={root}
      className="desktop-notice"
      onMouseEnter={() => setPaused(true)}
      onMouseLeave={() => setPaused(false)}
      onFocusCapture={() => setPaused(true)}
      onBlurCapture={(event) => {
        if (!event.currentTarget.contains(event.relatedTarget)) setPaused(false);
      }}
    >
      <header data-tauri-drag-region>
        <span className="desktop-notice-brand" data-tauri-drag-region>
          <img src={appIcon} width={16} height={16} alt="" />
          GitHubSP
          <time>{relativeTime(latest.createdAt)}</time>
        </span>
        <Button size="icon" variant="ghost" aria-label="关闭提醒" onClick={hide}>
          <X size={16} />
        </Button>
      </header>
      <section className={`desktop-notice-main is-${info.tone}`}>
        <span className="desktop-notice-icon" aria-hidden="true">
          <Icon size={22} strokeWidth={2} />
        </span>
        <div className="desktop-notice-copy">
          <strong>{info.title}</strong>
          <p className="desktop-notice-name" title={info.name}>
            {info.name}
          </p>
          {info.meta.length > 0 && <p className="desktop-notice-meta">{info.meta.join(" · ")}</p>}
        </div>
      </section>
      <div className="desktop-notice-actions">
        {info.tone === "done" && info.task && (
          <Button
            variant="default"
            size="sm"
            disabled={busy}
            onClick={() =>
              run(async () => {
                await request("openDirectory", { taskId: info.task!.id });
                await request("hideNotice");
              })
            }
          >
            <FolderOpen size={14} />
            打开文件夹
          </Button>
        )}
        <Button
          variant={info.tone === "done" && info.task ? "outline" : "default"}
          size="sm"
          disabled={busy}
          onClick={() => open(latest)}
        >
          {info.tone === "update" ? "查看更新" : "在 GitHubSP 中查看"}
          <ArrowUpRight size={14} />
        </Button>
      </div>
      {rest.length > 0 && (
        <div className="desktop-notice-more">
          <span>还有 {rest.length} 条提醒</span>
          {rest.slice(0, 2).map((notice) => {
            const item = describe(notice, snapshot);
            return (
              <button key={notice.id} onClick={() => open(notice)} title={item.name}>
                <i className={`is-${item.tone}`} aria-hidden="true" />
                <b>{item.title}</b>
                <span>{item.name}</span>
              </button>
            );
          })}
        </div>
      )}
      {message?.kind === "error" && <p className="desktop-notice-error">{message.text}</p>}
      <div
        key={`${key}-${paused}`}
        className={`desktop-notice-timer ${paused ? "is-paused" : ""}`}
        style={{ animationDuration: `${VISIBLE_MS}ms` }}
        aria-hidden="true"
      />
    </div>
  );
}
