import { PageHeader } from "@/components/PageActivity";
import { useEffect, useMemo, useRef, useState } from "react";
import {
  ArrowDownToLine,
  ArrowDown,
  ArrowUp,
  Copy,
  ChevronRight,
  FolderOpen,
  Pause,
  Play,
  Search,
  Trash2,
  X,
  Zap,
} from "lucide-react";
import { navigate, request, useWorkspace } from "@/lib/api";
import {
  bytes,
  date,
  duration,
  running,
  statusLabels,
  terminal,
  type Task,
} from "@/lib/model";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Empty, useConfirm, useJob } from "@/components/common";
import { TaskProgress } from "@/components/TaskProgress";

export function Queue() {
  const { snapshot, connected } = useWorkspace();
  const [filter, setFilter] = useState("all");
  const [query, setQuery] = useState("");
  const [selection, select] = useState<string | null>(null);
  const detailTrigger = useRef<HTMLButtonElement | null>(null);
  const detailClose = useRef<HTMLButtonElement | null>(null);
  const restoreTriggerFocus = useRef(false);
  const { busy, run } = useJob();
  const confirm = useConfirm();
  const tasks = useMemo(
    () =>
      [...(snapshot?.tasks ?? [])].sort(
        (a, b) => b.createdAt - a.createdAt || b.id.localeCompare(a.id),
      ),
    [snapshot?.tasks],
  );
  const visible = useMemo(
    () =>
      tasks.filter(
        (task) =>
          (filter === "all" ||
            (filter === "active"
              ? running(task.status) || task.status === "queued"
              : filter === "paused"
                ? task.status === "paused"
                : task.status === filter)) &&
          `${task.filename} ${task.repository ?? ""} ${task.tag ?? ""}`
            .toLowerCase()
            .includes(query.toLowerCase()),
      ),
    [tasks, filter, query],
  );
  const selected = visible.find((task) => task.id === selection);
  useEffect(() => {
    if (selection && !visible.some((task) => task.id === selection))
      select(null);
  }, [visible, selection]);
  useEffect(() => {
    if (selected) detailClose.current?.focus({ preventScroll: true });
    else if (restoreTriggerFocus.current) {
      // 窄窗口先恢复列表可见性，再把焦点还给任务按钮。
      detailTrigger.current?.focus({ preventScroll: true });
      restoreTriggerFocus.current = false;
    }
  }, [selected?.id]);
  const closeDetails = () => {
    restoreTriggerFocus.current = true;
    select(null);
  };
  const act = (task: Task, action: string) =>
    run(async () => {
      if (
        ["cancel", "remove"].includes(action) &&
        !(await confirm(
          action === "cancel" ? "取消这个下载？" : "移除这条记录？",
          action === "cancel"
            ? "下载将停止，该任务的续传分片会被清理。"
            : "只移除任务记录，已经下载的文件会保留。",
          "确认操作",
        ))
      )
        return;
      await request("action", { id: task.id, action });
    });
  return (
    <section className="page queue-page" tabIndex={-1}>
      <PageHeader>
        <div>
          <h1>
            下载队列
            <span className="count-label">{snapshot?.totalTasks ?? 0}</span>
          </h1>
        </div>
        <Button variant="default" onClick={() => navigate("new")}>
          <ArrowDownToLine size={16} />
          新建下载
        </Button>
      </PageHeader>
      <div className="queue-summary">
        {snapshot?.settings.limitKib ? (
          <span className="queue-limit">
            限速 {bytes(snapshot.settings.limitKib * 1024)}/s
          </span>
        ) : null}
        <div>
          <span className="live-dot" />
          <strong>{tasks.filter((t) => running(t.status)).length}</strong>{" "}
          正在处理
        </div>
        <div>
          <strong>{tasks.filter((t) => t.status === "queued").length}</strong>{" "}
          等待中
        </div>
        <div className="summary-speed">
          {bytes(tasks.reduce((sum, t) => sum + t.speed, 0))}
          <span>/s</span>
        </div>
      </div>
      <div className={`workbench ${selected ? "has-inspector" : ""}`}>
        <div className="task-list-pane">
          <div className="list-toolbar">
            <div className="tabs" role="tablist" aria-label="筛选下载任务">
              {[
                ["all", "全部"],
                ["active", "进行中"],
                ["paused", "已暂停"],
                ["failed", "失败"],
                ["completed", "已完成"],
              ].map(([key, label]) => (
                <button
                  role="tab"
                  aria-selected={filter === key}
                  className={filter === key ? "selected" : ""}
                  key={key}
                  onClick={() => setFilter(key)}
                >
                  {label}
                </button>
              ))}
            </div>
            <label className="search-box">
              <Search size={15} />
              <Input
                aria-label="搜索任务"
                placeholder="搜索任务"
                value={query}
                onChange={(e) => setQuery(e.target.value)}
              />
            </label>
          </div>
          <div className="bulk-toolbar">
            <Button
              size="sm"
              variant="ghost"
              disabled={
                !connected ||
                busy ||
                !tasks.some((t) => ["paused", "failed"].includes(t.status))
              }
              onClick={() =>
                run(() => request("batchAction", { action: "resume" }))
              }
            >
              <Play size={13} />
              全部继续
            </Button>
            <Button
              size="sm"
              variant="ghost"
              disabled={
                !connected ||
                busy ||
                !tasks.some((t) => running(t.status) || t.status === "queued")
              }
              onClick={() =>
                run(() => request("batchAction", { action: "pause" }))
              }
            >
              <Pause size={13} />
              全部暂停
            </Button>
            <span />
            <Button
              size="sm"
              variant="ghost"
              disabled={!connected || busy || !snapshot?.completedTasks}
              onClick={() =>
                run(async () => {
                  if (
                    await confirm(
                      "清空已完成记录？",
                      "将移除全部已完成的任务和历史记录，下载文件保留。",
                      "清空记录",
                    )
                  )
                    await request("batchAction", { action: "clear" });
                })
              }
            >
              <Trash2 size={13} />
              清空已完成
            </Button>
          </div>
          <div className="task-list">
            {visible.length === 0 ? (
              <Empty
                title={
                  query || filter !== "all" ? "没有匹配的任务" : "暂无下载任务"
                }
                description={
                  query || filter !== "all"
                    ? "试试其他关键词或任务状态。"
                    : "粘贴 GitHub 仓库、Release 页面或附件直链。"
                }
                action={
                  !query &&
                  filter === "all" && (
                    <Button onClick={() => navigate("new")}>添加下载</Button>
                  )
                }
              />
            ) : (
              visible.map((task) => (
                <article
                  className={`task-row ${selected?.id === task.id ? "is-selected" : ""}`}
                  key={task.id}
                >
                  <div className={`file-icon file-${task.status}`}>
                    <ArrowDownToLine size={19} />
                  </div>
                  <div className="task-row-main">
                    <div className="task-row-heading">
                      <button
                        className="task-name-button"
                        title={task.filename}
                        aria-label={`查看 ${task.filename} 的任务详情`}
                        aria-expanded={selected?.id === task.id}
                        aria-controls={
                          selected?.id === task.id
                            ? "task-inspector"
                            : undefined
                        }
                        onClick={(event) => {
                          detailTrigger.current = event.currentTarget;
                          select(task.id);
                        }}
                      >
                        {task.filename}
                      </button>
                    </div>
                    <div className="task-origin">
                      {task.repository || "GitHub Release"}
                      {task.tag && <> · {task.tag}</>}
                    </div>
                    <TaskProgress task={task} />
                  </div>
                  <div className="task-row-tail">
                    <span className={`status status-${task.status}`}>
                      {statusLabels[task.status]}
                    </span>
                    <span className="numeric">
                      {task.speed > 0
                        ? `${bytes(task.speed)}/s`
                        : task.status === "queued"
                          ? `队列第 ${task.queuePosition || "—"} 位`
                          : "—"}
                    </span>
                    <Button
                      variant="ghost"
                      size="icon"
                      aria-label={`${["paused", "failed", "waiting_network"].includes(task.status) ? "继续" : "暂停"} ${task.filename}`}
                      disabled={
                        !connected ||
                        busy ||
                        terminal(task.status) ||
                        ["pausing", "cancelling"].includes(task.status)
                      }
                      onClick={(e) => {
                        e.stopPropagation();
                        act(
                          task,
                          ["paused", "failed", "waiting_network"].includes(
                            task.status,
                          )
                            ? "resume"
                            : "pause",
                        );
                      }}
                    >
                      {["paused", "failed", "waiting_network"].includes(
                        task.status,
                      ) ? (
                        <Play size={15} />
                      ) : (
                        <Pause size={15} />
                      )}
                    </Button>
                  </div>
                  <ChevronRight
                    size={15}
                    className="task-detail-hint"
                    aria-hidden="true"
                  />
                </article>
              ))
            )}
          </div>
          <footer className="list-footer">
            <span>显示 {visible.length} 个任务</span>
            <button onClick={() => navigate("history")}>查看完整历史 →</button>
          </footer>
        </div>
        {selected && (
          <aside
            className="inspector"
            id="task-inspector"
            aria-label="任务详情"
            onKeyDown={(event) => {
              if (event.key === "Escape" && !event.defaultPrevented) {
                event.stopPropagation();
                closeDetails();
              }
            }}
          >
            <div className="inspector-title">
              任务详情
              <span className={`status status-${selected.status}`}>
                {statusLabels[selected.status]}
              </span>
              <Button
                ref={detailClose}
                size="icon"
                variant="ghost"
                aria-label="收起任务详情"
                onClick={closeDetails}
              >
                <X size={17} />
              </Button>
            </div>
            <div className="inspector-file">
              <h2>{selected.filename}</h2>
              <p>{selected.repository || "GitHub Release 附件"}</p>
            </div>
            <TaskProgress task={selected} />
            <div className="detail-stats">
              <div>
                <small>下载速度</small>
                <strong>
                  {selected.speed ? `${bytes(selected.speed)}/s` : "—"}
                </strong>
              </div>
              <div>
                <small>预计剩余</small>
                <strong>{duration(selected.eta)}</strong>
              </div>
            </div>
            {selected.error && (
              <div className="detail-error">
                {selected.error}
                {selected.failure?.action && (
                  <small>{selected.failure.action}</small>
                )}
              </div>
            )}
            {selected.retryInfo && (
              <div className="detail-note">
                第 {selected.retryInfo.attempt}/{selected.retryInfo.maxAttempts}{" "}
                次重试 · {duration(selected.retryInfo.retryInMs / 1000)} 后
                <br />
                {selected.retryInfo.reason}
              </div>
            )}
            {selected.recoveryInfo && (
              <div className="detail-note">
                {selected.recoveryInfo.waitingForSlot
                  ? "等待其他任务让出下载位置"
                  : `${duration(selected.recoveryInfo.retryInMs / 1000)} 后尝试恢复网络`}{" "}
                · 剩余额度 {duration(selected.recoveryInfo.remainingMs / 1000)}
              </div>
            )}
            <dl className="details">
              <dt>保存位置</dt>
              <dd className="break-all">{selected.directory}</dd>
              <dt>添加时间</dt>
              <dd>{date(selected.createdAt)}</dd>
              <dt>校验状态</dt>
              <dd>
                {{
                  verified: "SHA-256 校验通过",
                  unverified: "无官方摘要",
                  pending: "等待校验",
                  failed: "校验失败",
                }[selected.verification] || selected.verification}
              </dd>
              <dt>已用时间</dt>
              <dd>
                {selected.elapsedIsPartial ? "至少 " : ""}
                {duration(
                  selected.elapsedMs == null ? null : selected.elapsedMs / 1000,
                )}
              </dd>
              <dt>下载线路</dt>
              <dd>
                <select
                  aria-label="任务下载线路"
                  value={selected.preferredRoute || ""}
                  disabled={busy || terminal(selected.status)}
                  onChange={(e) => {
                    const route = e.target.value || null;
                    run(async () => {
                      const restart = selected.downloaded > 0;
                      if (
                        restart &&
                        !(await confirm(
                          "切换线路并重新下载？",
                          `新线路将从头下载 ${bytes(selected.total)}，不拼接旧线路的分片。`,
                          "换线重下",
                        ))
                      )
                        return;
                      await request("route", {
                        id: selected.id,
                        route,
                        restart,
                      });
                    });
                  }}
                >
                  <option value="">自动检测</option>
                  <option value="github">GitHub 直连</option>
                  <option value="gh-proxy">GH-Proxy</option>
                  <option value="ghproxy-net">ghproxy.net</option>
                </select>
              </dd>
            </dl>
            {selected.routeSuggestion && (
              <div className="route-suggestion">
                <strong>
                  <Zap size={15} />
                  有更快的线路
                </strong>
                <p>
                  {selected.routeSuggestion.routeName} · 预计{" "}
                  {duration(selected.routeSuggestion.suggestedSeconds)}
                  ，需要重下 {bytes(selected.routeSuggestion.restartBytes)}。
                </p>
                <div>
                  <Button
                    size="sm"
                    disabled={busy}
                    onClick={() =>
                      run(async () => {
                        if (
                          await confirm(
                            "使用建议线路？",
                            "将重新检测线路并从头下载，已有文件不会被覆盖。",
                            "换线重下",
                          )
                        )
                          await request("suggestion", {
                            id: selected.id,
                            suggestionId: selected.routeSuggestion!.id,
                            apply: true,
                          });
                      })
                    }
                  >
                    换线重下
                  </Button>
                  <Button
                    size="sm"
                    variant="ghost"
                    disabled={busy}
                    onClick={() =>
                      run(() =>
                        request("suggestion", {
                          id: selected.id,
                          suggestionId: selected.routeSuggestion!.id,
                          apply: false,
                        }),
                      )
                    }
                  >
                    继续当前线路
                  </Button>
                </div>
              </div>
            )}
            {!!selected.routeFailures?.length && (
              <details className="detail-note">
                <summary>线路失败详情</summary>
                {selected.routeFailures.map((f, i) => (
                  <p key={i}>
                    {f.routeName}：{f.message}
                  </p>
                ))}
              </details>
            )}
            {selected.status === "queued" && (
              <div className="queue-order">
                <Button
                  size="sm"
                  disabled={busy}
                  onClick={() =>
                    run(() =>
                      request("reorder", {
                        id: selected.id,
                        direction: "up",
                      }),
                    )
                  }
                >
                  <ArrowUp size={13} />
                  提前
                </Button>
                <Button
                  size="sm"
                  disabled={busy}
                  onClick={() =>
                    run(() =>
                      request("reorder", {
                        id: selected.id,
                        direction: "down",
                      }),
                    )
                  }
                >
                  <ArrowDown size={13} />
                  延后
                </Button>
                <Button
                  size="sm"
                  disabled={busy}
                  onClick={() =>
                    run(() =>
                      request("reorder", {
                        id: selected.id,
                        direction: "top",
                      }),
                    )
                  }
                >
                  优先下载
                </Button>
              </div>
            )}
            <div className="inspector-actions">
              <Button
                disabled={busy}
                onClick={() =>
                  run(() => request("openDirectory", { taskId: selected.id }))
                }
              >
                <FolderOpen size={14} />
                打开目录
              </Button>
              <Button
                disabled={busy}
                onClick={() =>
                  run(() => request("copy", { text: selected.url }))
                }
              >
                <Copy size={14} />
                复制链接
              </Button>
              <Button
                variant="ghost"
                className="danger-text"
                disabled={
                  busy || ["pausing", "cancelling"].includes(selected.status)
                }
                onClick={() =>
                  act(selected, terminal(selected.status) ? "remove" : "cancel")
                }
              >
                <X size={14} />
                {terminal(selected.status) ? "移除记录" : "取消下载"}
              </Button>
            </div>
          </aside>
        )}
      </div>
    </section>
  );
}
