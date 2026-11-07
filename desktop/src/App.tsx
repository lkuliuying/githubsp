import { useEffect, useState } from "react";
import { isTauri } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  ArrowDownToLine,
  Bell,
  Check,
  Minus,
  Moon,
  Square,
  Star,
  Sun,
  X,
} from "lucide-react";
import {
  connect,
  dismissMessage,
  navigate,
  notify,
  request,
  useWorkspace,
} from "./lib/api";
import { Navigation } from "./components/Navigation";
import { PageHost } from "./components/PageHost";
import { Button } from "./components/ui/button";
import { Busy, useConfirm, useJob } from "./components/common";
import { Queue } from "./views/Queue";
import { NewDownload } from "./views/NewDownload";
import { History } from "./views/History";
import { Favorites } from "./views/Favorites";
import { Routes } from "./views/Routes";
import { hasUnsavedSettings, Settings } from "./views/Settings";

const pages = {
  queue: Queue,
  new: NewDownload,
  history: History,
  favorites: Favorites,
  routes: Routes,
  settings: Settings,
};
export function App() {
  const { snapshot, info, view, connected, error, message } = useWorkspace();
  const [dark, setDark] = useState(() => {
    try {
      return localStorage.getItem("githubsp.theme") === "dark";
    } catch {
      return false;
    }
  });
  const [notices, showNotices] = useState(false);
  const [systemReduced, setSystemReduced] = useState(
    () => matchMedia("(prefers-reduced-motion: reduce)").matches,
  );
  useEffect(() => {
    const media = matchMedia("(prefers-reduced-motion: reduce)");
    const changed = () => setSystemReduced(media.matches);
    media.addEventListener("change", changed);
    return () => media.removeEventListener("change", changed);
  }, []);
  useEffect(() => showNotices(false), [view]);
  const [connectionAttempt, setConnectionAttempt] = useState(0);
  const confirm = useConfirm();
  const { run } = useJob();
  useEffect(() => {
    const controller = new AbortController();
    void connect(controller.signal);
    return () => controller.abort();
  }, [connectionAttempt]);
  useEffect(() => {
    document.documentElement.classList.toggle("dark", dark);
    try {
      localStorage.setItem("githubsp.theme", dark ? "dark" : "light");
    } catch {
      notify("主题偏好无法保存，将在本次运行中生效。");
    }
  }, [dark]);
  useEffect(() => {
    document.documentElement.classList.toggle(
      "reduce-motion",
      !!snapshot?.settings.reduceMotion ||
        !!info?.systemReduceMotion ||
        systemReduced,
    );
  }, [
    snapshot?.settings.reduceMotion,
    info?.systemReduceMotion,
    systemReduced,
  ]);
  useEffect(() => {
    if (!message || message.kind === "error") return;
    const timer = setTimeout(dismissMessage, 6000);
    return () => clearTimeout(timer);
  }, [message]);
  useEffect(() => {
    const keydown = (event: KeyboardEvent) => {
      if (
        (event.ctrlKey || event.metaKey) &&
        event.key.toLowerCase() === "n" &&
        !event.isComposing
      ) {
        if (document.querySelector('[role="dialog"]')) return;
        event.preventDefault();
        navigate("new");
      }
    };
    document.addEventListener("keydown", keydown);
    return () => document.removeEventListener("keydown", keydown);
  }, []);
  const reduced =
    !!snapshot?.settings.reduceMotion ||
    !!info?.systemReduceMotion ||
    systemReduced;
  return (
    <div className="app-shell">
      <Navigation reduced={reduced} />
      <main className="main-workspace">
        <div className="titlebar" data-tauri-drag-region>
          <div id="page-header-slot" data-tauri-drag-region />
          <div className="window-tools">
            <Button
              variant="ghost"
              size="icon"
              aria-label={dark ? "切换浅色主题" : "切换深色主题"}
              onClick={() => setDark(!dark)}
            >
              {dark ? <Sun size={15} /> : <Moon size={15} />}
            </Button>
            <Button
              variant="ghost"
              size="icon"
              aria-label="查看通知"
              onClick={() => showNotices(!notices)}
            >
              <Bell size={15} />
              {!!snapshot?.notices.length && <i className="notification-dot" />}
            </Button>
            <span className="titlebar-separator" />
            {isTauri() && (
              <>
                <Button
                  variant="ghost"
                  size="icon"
                  aria-label="最小化"
                  onClick={() => run(() => getCurrentWindow().minimize())}
                >
                  <Minus size={15} />
                </Button>
                <Button
                  variant="ghost"
                  size="icon"
                  aria-label="最大化或还原"
                  onClick={() => run(() => getCurrentWindow().toggleMaximize())}
                >
                  <Square size={12} />
                </Button>
                <Button
                  variant="ghost"
                  size="icon"
                  className="window-close"
                  aria-label="关闭主窗口"
                  onClick={() =>
                    run(async () => {
                      if (
                        hasUnsavedSettings() &&
                        !(await confirm(
                          "关闭前放弃未保存的设置？",
                          "下载进度仍会保存；未保存的设置更改会丢失。",
                          "继续关闭",
                        ))
                      )
                        return;
                      await request("close");
                    })
                  }
                >
                  <X size={16} />
                </Button>
              </>
            )}
          </div>
        </div>
        {error && (
          <div className="connection-banner" role="status">
            {error}
            <button
              onClick={() => {
                setConnectionAttempt((value) => value + 1);
              }}
            >
              重新连接
            </button>
          </div>
        )}
        {!snapshot && isTauri() && !error ? (
          <Busy text="正在连接…" />
        ) : (
          <PageHost view={view} pages={pages} />
        )}
      </main>
      {message && (
        <div
          className={`toast toast-${message.kind}`}
          role={message.kind === "error" ? "alert" : "status"}
        >
          <span>{message.text}</span>
          <Button
            size="icon"
            variant="ghost"
            aria-label="关闭消息"
            onClick={dismissMessage}
          >
            <X size={15} />
          </Button>
          {/服务已停止|迁移未完成|服务已停止|保存进度失败/.test(
            message.text,
          ) && (
            <Button size="sm" onClick={() => run(() => request("restart"))}>
              重新启动
            </Button>
          )}
        </div>
      )}
      {notices && (
        <aside className="notice-panel">
          <header>
            <h2>通知</h2>
            <Button
              variant="ghost"
              size="icon"
              aria-label="关闭通知"
              onClick={() => showNotices(false)}
            >
              <X size={16} />
            </Button>
          </header>
          {snapshot?.notices.length ? (
            <>
              <div className="notice-list">
                {snapshot.notices.map((notice) => (
                  <button
                    key={notice.id}
                    onClick={() => {
                      navigate(
                        notice.kind === "favorite_updated"
                          ? "favorites"
                          : "queue",
                      );
                      showNotices(false);
                    }}
                  >
                    <span className="notice-icon">
                      {notice.kind === "favorite_updated" ? (
                        <Star size={17} />
                      ) : (
                        <ArrowDownToLine size={17} />
                      )}
                    </span>
                    <div>
                      <strong>{notice.message}</strong>
                      <small>
                        {new Date(notice.createdAt).toLocaleString("zh-CN")}
                      </small>
                    </div>
                  </button>
                ))}
              </div>
              <Button
                variant="ghost"
                disabled={!connected}
                onClick={() => run(() => request("acknowledge"))}
              >
                <Check size={14} />
                清除通知
              </Button>
            </>
          ) : (
            <p className="muted">暂时没有新通知</p>
          )}
        </aside>
      )}
    </div>
  );
}
