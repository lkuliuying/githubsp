import { PageHeader, usePageActive } from "@/components/PageActivity";
import { useEffect, useMemo, useRef, useState, type Ref } from "react";
import {
  ArrowDownToLine,
  ArrowDown,
  ArrowUp,
  Copy,
  Link as LinkIcon,
  ChevronRight,
  FolderOpen,
  Pause,
  Play,
  RotateCcw,
  Search,
  Trash2,
  X,
  Zap,
} from "lucide-react";
import { navigate, notify, request, useWorkspace } from "@/lib/api";
import {
  bytes,
  date,
  duration,
  fileStateLabels,
  formatLimit,
  relativeTime,
  running,
  statusLabels,
  terminal,
  type HistoryPage,
  type Task,
  routeOptions,
} from "@/lib/model";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Menu } from "@/components/Menu";
import { Pager, clampPage, usePageSize } from "@/components/Pager";
import { detectSource } from "@/lib/source";
import { openRepository } from "./NewDownload";
import { Empty, ErrorBox, useConfirm, useJob } from "@/components/common";
import { TaskProgress } from "@/components/TaskProgress";

type Filter = "unfinished" | "active" | "paused" | "failed" | "done";
const resumable = (task: Task) =>
  ["paused", "failed", "waiting_network"].includes(task.status);
const matches: Record<Exclude<Filter, "done">, (task: Task) => boolean> = {
  unfinished: (task) => !terminal(task.status),
  active: (task) =>
    (running(task.status) && task.status !== "waiting_network") ||
    task.status === "queued",
  paused: (task) => ["paused", "waiting_network"].includes(task.status),
  failed: (task) => task.status === "failed",
};
const verificationLabels: Record<string, string> = {
  verified: "SHA-256 校验通过",
  unverified: "没有官方摘要",
  pending: "等待校验",
  failed: "校验失败",
};
/** 失败原因优先显示分类后的说明，其次是原始错误。 */
const failureText = (task: Task) => task.failure?.message || task.error || "";

export function Queue() {
  const visible = usePageActive();
  const { snapshot, connected } = useWorkspace();
  const [filter, setFilter] = useState<Filter>("unfinished");
  const [query, setQuery] = useState("");
  const [selection, select] = useState<string | null>(null);
  const detailTrigger = useRef<HTMLButtonElement | null>(null);
  const detailClose = useRef<HTMLButtonElement | null>(null);
  const restoreTriggerFocus = useRef(false);
  const { busy, run } = useJob();
  const confirm = useConfirm();

  // ---- 已完成：分页读取历史 ----
  const [doneStatus, setDoneStatus] = useState<"completed" | "cancelled">(
    "completed",
  );
  const [page, setPage] = useState(1);
  const [pageSize, setPageSize] = usePageSize("queue");
  const [history, setHistory] = useState<HistoryPage | null>(null);
  const [historyError, setHistoryError] = useState("");
  const [loading, setLoading] = useState(false);
  const [revision, refresh] = useState(0);
  useEffect(() => {
    if (filter !== "done" || !connected || !visible) return;
    let cancelled = false;
    setLoading(true);
    const timer = setTimeout(() => {
      request<HistoryPage>("history", { query, status: doneStatus, page, pageSize })
        .then((value) => {
          if (!cancelled) {
            setHistory(value);
            setHistoryError("");
          }
        })
        .catch((reason) => {
          if (!cancelled) setHistoryError(String(reason));
        })
        .finally(() => {
          if (!cancelled) setLoading(false);
        });
    }, 180);
    return () => {
      cancelled = true;
      clearTimeout(timer);
    };
  }, [filter, doneStatus, query, page, pageSize, revision, snapshot?.historyRevision, connected, visible]);

  const tasks = useMemo(
    () =>
      [...(snapshot?.tasks ?? [])].sort(
        (a, b) => b.createdAt - a.createdAt || b.id.localeCompare(a.id),
      ),
    [snapshot?.tasks],
  );
  const unfinished = tasks.filter(matches.unfinished);
  // 后端的 queuePosition 是排序序号而非名次，这里换算成等待中的第几位。
  const waiting = tasks
    .filter((task) => task.status === "queued")
    .sort((a, b) => (a.queuePosition ?? 0) - (b.queuePosition ?? 0));
  const queueRank = (task: Task) =>
    waiting.findIndex((item) => item.id === task.id) + 1 || "—";
  const counts = {
    unfinished: unfinished.length,
    active: tasks.filter(matches.active).length,
    paused: tasks.filter(matches.paused).length,
    failed: tasks.filter(matches.failed).length,
    done: snapshot?.completedTasks ?? 0,
  };
  const visibleTasks = useMemo(
    () =>
      filter === "done"
        ? []
        : tasks.filter(
            (task) =>
              matches[filter](task) &&
              `${task.filename} ${task.repository ?? ""} ${task.tag ?? ""}`
                .toLowerCase()
                .includes(query.toLowerCase()),
          ),
    [tasks, filter, query],
  );
  const doneTasks = filter === "done" ? (history?.items ?? []) : [];
  // 记住已经出现过的任务；之后新加入的任务短暂高亮。首次加载时不高亮。
  const known = useRef<Set<string> | null>(null);
  const [fresh, setFresh] = useState<Set<string>>(() => new Set());
  useEffect(() => {
    if (!snapshot) return;
    const ids = snapshot.tasks.map((task) => task.id);
    if (!known.current) {
      known.current = new Set(ids);
      return;
    }
    const added = ids.filter((id) => !known.current!.has(id));
    added.forEach((id) => known.current!.add(id));
    if (!added.length) return;
    setFresh((old) => new Set([...old, ...added]));
    setTimeout(
      () =>
        setFresh((old) => {
          const next = new Set(old);
          added.forEach((id) => next.delete(id));
          return next;
        }),
      1700,
    );
  }, [snapshot?.tasks]);
  // 未完成列表在前端分页；已完成列表由后端按页返回。
  const listPage = clampPage(page, visibleTasks.length, pageSize);
  const pagedTasks = visibleTasks.slice((listPage - 1) * pageSize, listPage * pageSize);
  const selected =
    visibleTasks.find((task) => task.id === selection) ??
    doneTasks.find((item) => item.task.id === selection)?.task;
  useEffect(() => {
    if (selection && !selected) select(null);
  }, [selected, selection]);
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
  const changeFilter = (next: Filter) => {
    setFilter(next);
    setPage(1);
    select(null);
  };
  const act = (task: Task, action: string) =>
    run(async () => {
      if (
        ["cancel", "remove"].includes(action) &&
        !(await confirm(
          action === "cancel" ? "取消这个下载？" : "移除这条记录？",
          action === "cancel"
            ? "下载将停止，已下载的部分会被清理。"
            : "只移除记录，已经下载的文件会保留。",
          action === "cancel" ? "取消下载" : "移除记录",
        ))
      )
        return;
      await request("action", { id: task.id, action });
      if (action === "remove") refresh((value) => value + 1);
    });
  const redownload = (task: Task) =>
    run(async () => {
      const result = await request<{
        items: { status: string; message: string | null }[];
      }>("create", { urls: [task.url], directory: task.directory, route: null });
      const failure = result.items.find((item) => item.status !== "created");
      if (failure)
        throw new Error(failure.message || "没能加入队列，请检查链接和保存位置");
      notify("已重新加入下载队列", "success");
    });
  const openFolder = (task: Task) =>
    run(() => request("openDirectory", { taskId: task.id }));
  const nameButton = (task: Task) => (
    <button
      className="task-name-button"
      title={task.filename}
      aria-label={`查看 ${task.filename} 的任务详情`}
      aria-expanded={selected?.id === task.id}
      aria-controls={selected?.id === task.id ? "task-inspector" : undefined}
      onClick={(event) => {
        detailTrigger.current = event.currentTarget;
        select(task.id);
      }}
    >
      {task.filename}
    </button>
  );
  const origin = (task: Task, when?: number) => (
    <span className="task-origin">
      {task.repository || "GitHub Release"}
      {task.tag && <> · {task.tag}</>}
      {when && (
        <>
          {" · "}
          <time title={date(when)}>{relativeTime(when)}</time>
        </>
      )}
    </span>
  );

  const tabs: [Filter, string][] = [
    ["unfinished", "未完成"],
    ["active", "进行中"],
    ["paused", "已暂停"],
    ["failed", "失败"],
    ["done", "已完成"],
  ];
  return (
    <section className="page queue-page" tabIndex={-1}>
      <PageHeader>
        <div>
          <h1>
            下载队列
            {counts.unfinished > 0 && (
              <span className="count-label" title="未完成的任务">
                {counts.unfinished}
              </span>
            )}
          </h1>
        </div>
        <div className="header-actions">
          <div className="header-stats" aria-label="下载概况">
            <span>
              <i className="live-dot" />
              <strong>{tasks.filter((t) => running(t.status)).length}</strong>
              正在处理
            </span>
            <span>
              <strong>{tasks.filter((t) => t.status === "queued").length}</strong>
              等待中
            </span>
            <span className="header-speed">
              {bytes(tasks.reduce((sum, t) => sum + t.speed, 0))}/s
            </span>
            {snapshot?.settings.limitKib ? (
              <span title="可在设置 → 下载中修改">
                限速 {formatLimit(snapshot.settings.limitKib)}
              </span>
            ) : null}
          </div>
          <Button variant="default" onClick={() => navigate("new")}>
            <ArrowDownToLine size={16} />
            <span className="collapsible-label">新建下载</span>
          </Button>
        </div>
      </PageHeader>
      <div className="queue-top">
        <QuickDownload />
      </div>
      <div className={`workbench ${selected ? "has-inspector" : ""}`}>
        <div className="task-list-pane">
          <div className="list-toolbar">
            <div className="tabs" role="tablist" aria-label="筛选下载任务">
              {tabs.map(([key, label]) => (
                <button
                  role="tab"
                  aria-selected={filter === key}
                  className={filter === key ? "selected" : ""}
                  key={key}
                  onClick={() => changeFilter(key)}
                >
                  {label}
                  {counts[key] > 0 && (
                    <span
                      className={`tab-count ${key === "failed" ? "is-danger" : ""}`}
                    >
                      {counts[key]}
                    </span>
                  )}
                </button>
              ))}
            </div>
            <div className="list-tools">
            {filter === "done" ? (
              <>
                <div className="segmented" role="group" aria-label="已完成类型">
                  {(
                    [
                      ["completed", "下载完成"],
                      ["cancelled", "已取消"],
                    ] as const
                  ).map(([key, label]) => (
                    <button
                      key={key}
                      aria-pressed={doneStatus === key}
                      className={doneStatus === key ? "selected" : ""}
                      onClick={() => {
                        setDoneStatus(key);
                        setPage(1);
                      }}
                    >
                      {label}
                    </button>
                  ))}
                </div>
                <Button
                  size="sm"
                  variant="ghost"
                  disabled={!connected || busy || !snapshot?.completedTasks}
                  onClick={() =>
                    run(async () => {
                      if (
                        await confirm(
                          "清空已完成记录？",
                          "所有下载完成的记录都会移除，下载好的文件会保留。",
                          "清空记录",
                        )
                      ) {
                        await request("batchAction", { action: "clear" });
                        refresh((value) => value + 1);
                      }
                    })
                  }
                >
                  <Trash2 size={13} />
                  清空记录
                </Button>
              </>
            ) : (
              <>
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
              </>
            )}
            </div>
            <label className="search-box">
              <Search size={15} />
              <Input
                aria-label="搜索任务"
                placeholder="搜索任务"
                value={query}
                onChange={(e) => {
                  setQuery(e.target.value);
                  setPage(1);
                }}
              />
            </label>
          </div>
          {filter === "done" && historyError && <ErrorBox text={historyError} />}
          <div className="task-list">
            {filter === "done" ? (
              doneTasks.length ? (
                doneTasks.map(({ task, fileState }) => (
                  <article
                    className={`task-row is-done ${selected?.id === task.id ? "is-selected" : ""}`}
                    key={task.id}
                  >
                    <div className={`file-icon file-${task.status}`}>
                      <ArrowDownToLine size={19} />
                    </div>
                    <div className="task-row-main">
                      <div className="task-row-heading">
                        {nameButton(task)}
                        {origin(task, task.completedAt || task.createdAt)}
                      </div>
                      <div
                        className={`task-row-note ${fileState !== "present" && task.status === "completed" ? "is-warning" : ""}`}
                      >
                        {bytes(task.total ?? task.downloaded)}
                        {task.status === "completed" &&
                          fileState !== "present" &&
                          ` · ${fileStateLabels[fileState] ?? "位置未确认"}`}
                      </div>
                    </div>
                    <div className="task-row-actions">
                      {task.status === "completed" && fileState === "present" ? (
                        <Button
                          size="sm"
                          disabled={busy}
                          onClick={() => openFolder(task)}
                        >
                          <FolderOpen size={14} />
                          打开文件夹
                        </Button>
                      ) : (
                        <Button
                          size="sm"
                          disabled={busy || !connected}
                          onClick={() => redownload(task)}
                        >
                          <RotateCcw size={14} />
                          重新下载
                        </Button>
                      )}
                      <Menu
                        label={`${task.filename} 的更多操作`}
                        disabled={busy}
                        items={[
                          task.status === "completed" &&
                            fileState === "present" && {
                              label: "重新下载",
                              icon: <RotateCcw size={14} />,
                              onSelect: () => redownload(task),
                            },
                          {
                            label: "复制下载链接",
                            icon: <Copy size={14} />,
                            onSelect: () =>
                              run(() => request("copy", { text: task.url })),
                          },
                          {
                            label: "移除记录",
                            icon: <Trash2 size={14} />,
                            danger: true,
                            onSelect: () => act(task, "remove"),
                          },
                        ]}
                      />
                    </div>
                  </article>
                ))
              ) : (
                !loading && (
                  <Empty
                    title={query ? "没有匹配的记录" : "还没有记录"}
                    description={
                      query
                        ? "换个关键词试试。"
                        : doneStatus === "completed"
                          ? "下载完成的文件会出现在这里，可以打开文件夹或重新下载。"
                          : "取消的下载会出现在这里。"
                    }
                  />
                )
              )
            ) : visibleTasks.length === 0 ? (
              <Empty
                title={
                  query || filter !== "unfinished"
                    ? "没有匹配的任务"
                    : "没有正在进行的下载"
                }
                description={
                  query || filter !== "unfinished"
                    ? "换个关键词或筛选条件试试。"
                    : "粘贴 GitHub 仓库、Release 页面或附件链接就能开始下载。"
                }
                action={
                  !query &&
                  filter === "unfinished" && (
                    <Button onClick={() => navigate("new")}>新建下载</Button>
                  )
                }
              />
            ) : (
              pagedTasks.map((task) => (
                <article
                  className={`task-row ${selected?.id === task.id ? "is-selected" : ""} ${fresh.has(task.id) ? "is-new" : ""}`}
                  key={task.id}
                >
                  <div className={`file-icon file-${task.status}`}>
                    <ArrowDownToLine size={19} />
                  </div>
                  <div className="task-row-main">
                    <div className="task-row-heading">
                      {nameButton(task)}
                      {origin(task)}
                    </div>
                    <TaskProgress task={task} />
                    {task.status === "failed" && failureText(task) && (
                      <div className="task-row-note is-danger" title={failureText(task)}>
                        {failureText(task)}
                      </div>
                    )}
                  </div>
                  <div className="task-row-tail">
                    <span className={`status status-${task.status}`}>
                      {statusLabels[task.status]}
                    </span>
                    <span className="numeric">
                      {task.speed > 0
                        ? `${bytes(task.speed)}/s`
                        : task.status === "queued"
                          ? `排第 ${queueRank(task)} 位`
                          : "—"}
                    </span>
                    {task.status === "failed" ? (
                      <Button
                        size="sm"
                        aria-label={`重试 ${task.filename}`}
                        disabled={!connected || busy}
                        onClick={(e) => {
                          e.stopPropagation();
                          act(task, "resume");
                        }}
                      >
                        <RotateCcw size={13} />
                        <span className="collapsible-label">重试</span>
                      </Button>
                    ) : (
                      <Button
                        variant="ghost"
                        size="icon"
                        aria-label={`${resumable(task) ? "继续" : "暂停"} ${task.filename}`}
                        disabled={
                          !connected ||
                          busy ||
                          ["pausing", "cancelling"].includes(task.status)
                        }
                        onClick={(e) => {
                          e.stopPropagation();
                          act(task, resumable(task) ? "resume" : "pause");
                        }}
                      >
                        {resumable(task) ? <Play size={15} /> : <Pause size={15} />}
                      </Button>
                    )}
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
          {filter === "done" ? (
            <Pager
              page={history?.page ?? page}
              total={history?.total ?? 0}
              pageSize={pageSize}
              onPage={setPage}
              onPageSize={setPageSize}
              disabled={loading}
            />
          ) : (
            <Pager
              page={listPage}
              total={visibleTasks.length}
              pageSize={pageSize}
              onPage={setPage}
              onPageSize={setPageSize}
              unit="个任务"
              extra={
                counts.done > 0 && (
                  <button className="pager-link" onClick={() => changeFilter("done")}>
                    已完成 {counts.done} 个 →
                  </button>
                )
              }
            />
          )}
        </div>
        {selected && (
          <TaskInspector
            task={selected}
            busy={busy}
            closeRef={detailClose}
            onClose={closeDetails}
            onAct={act}
            onRedownload={redownload}
            run={run}
          />
        )}
      </div>
    </section>
  );
}

/** 队列顶部的快速下载：附件链接直接加入队列，仓库地址转到新建下载挑选版本。 */
function QuickDownload() {
  const { snapshot, connected } = useWorkspace();
  const [text, setText] = useState("");
  const [directory, setDirectory] = useState<string | null>(null);
  const target = directory ?? snapshot?.lastDirectory ?? null;
  const { busy, run, active } = useJob();
  const detected = detectSource(text);
  const submit = () =>
    run(async () => {
      if (detected.kind === "repository") {
        openRepository(detected.lines[0]);
        setText("");
        return;
      }
      if (detected.kind !== "links") {
        notify("请粘贴附件链接（每行一个），或者单独粘贴一个仓库地址。");
        return;
      }
      if (!target) {
        // 还没有用过任何保存位置时，转到新建下载选择目录。
        openRepository(text, false);
        setText("");
        return;
      }
      const result = await request<{
        items: { status: string; message: string | null; input: string }[];
      }>("create", { urls: detected.lines, directory: target, route: null });
      if (!active.current) return;
      const created = result.items.filter((item) => item.status === "created");
      const failed = result.items.filter((item) => item.status !== "created");
      if (created.length) setText(failed.map((item) => item.input).join("\n"));
      if (failed.length)
        notify(
          `${created.length ? `已加入 ${created.length} 个，` : ""}${failed.length} 个没有加入：${failed[0].message ?? "请检查链接"}`,
          created.length ? "info" : "error",
        );
      else
        notify(
          `已加入下载队列，保存到「${target.split(/[\\/]/).filter(Boolean).pop() ?? target}」`,
          "success",
        );
    });
  return (
    <div className="quick-download">
      <LinkIcon size={15} aria-hidden="true" />
      <textarea
        aria-label="快速下载"
        rows={1}
        placeholder="粘贴附件链接直接下载，或粘贴仓库地址挑选版本"
        value={text}
        disabled={busy}
        spellCheck={false}
        onChange={(event) => setText(event.target.value)}
        onKeyDown={(event) => {
          if (event.key === "Enter" && !event.shiftKey && !event.nativeEvent.isComposing) {
            event.preventDefault();
            if (text.trim()) submit();
          }
        }}
      />
      {detected.kind === "links" && (
        <button
          className="quick-target"
          title={target ? `保存到 ${target}，点击更改` : "选择保存位置"}
          disabled={busy}
          onClick={() =>
            run(async () => {
              const path = await request<string | null>("chooseDirectory", {
                initial: target,
              });
              if (path && active.current) setDirectory(path);
            })
          }
        >
          <FolderOpen size={13} />
          {target ? target.split(/[\\/]/).filter(Boolean).pop() : "选择位置"}
        </button>
      )}
      <Button
        size="sm"
        variant="default"
        disabled={busy || !connected || !text.trim()}
        onClick={submit}
      >
        {detected.kind === "repository" ? "读取版本" : "下载"}
      </Button>
    </div>
  );
}

function TaskInspector({
  task: selected,
  busy,
  closeRef,
  onClose,
  onAct,
  onRedownload,
  run,
}: {
  task: Task;
  busy: boolean;
  closeRef: Ref<HTMLButtonElement>;
  onClose(): void;
  onAct(task: Task, action: string): void;
  onRedownload(task: Task): void;
  run(fn: () => Promise<unknown>): Promise<void>;
}) {
  const confirm = useConfirm();
  const { snapshot } = useWorkspace();
  const finished = terminal(selected.status);
  return (
    <aside
      className="inspector"
      id="task-inspector"
      aria-label="任务详情"
      onKeyDown={(event) => {
        if (event.key === "Escape" && !event.defaultPrevented) {
          event.stopPropagation();
          onClose();
        }
      }}
    >
      <div className="inspector-title">
        任务详情
        <span className={`status status-${selected.status}`}>
          {statusLabels[selected.status]}
        </span>
        <Button
          ref={closeRef}
          size="icon"
          variant="ghost"
          aria-label="收起任务详情"
          onClick={onClose}
        >
          <X size={17} />
        </Button>
      </div>
      <div className="inspector-file">
        <h2>{selected.filename}</h2>
        <p>
          {selected.repository || "GitHub Release 附件"}
          {selected.tag && ` · ${selected.tag}`}
        </p>
      </div>
      {!finished && <TaskProgress task={selected} />}
      {!finished && (
        <div className="detail-stats">
          <div>
            <small>下载速度</small>
            <strong>{selected.speed ? `${bytes(selected.speed)}/s` : "—"}</strong>
          </div>
          <div>
            <small>预计剩余</small>
            <strong>{duration(selected.eta)}</strong>
          </div>
        </div>
      )}
      {selected.error && (
        <div className="detail-error">
          {failureText(selected)}
          {selected.failure?.action && <small>{selected.failure.action}</small>}
        </div>
      )}
      {selected.retryInfo && (
        <div className="detail-note">
          第 {selected.retryInfo.attempt}/{selected.retryInfo.maxAttempts} 次重试 ·{" "}
          {duration(selected.retryInfo.retryInMs / 1000)} 后
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
        <dd className="break-all">{selected.finalPath ?? selected.directory}</dd>
        <dt>大小</dt>
        <dd>{bytes(selected.total ?? selected.downloaded)}</dd>
        <dt>添加时间</dt>
        <dd>{date(selected.createdAt)}</dd>
        {selected.completedAt && (
          <>
            <dt>完成时间</dt>
            <dd>{date(selected.completedAt)}</dd>
          </>
        )}
        <dt>校验状态</dt>
        <dd>{verificationLabels[selected.verification] || selected.verification}</dd>
        <dt>已用时间</dt>
        <dd>
          {selected.elapsedIsPartial ? "至少 " : ""}
          {duration(selected.elapsedMs == null ? null : selected.elapsedMs / 1000)}
        </dd>
        {!finished && (
          <>
            <dt>下载线路</dt>
            <dd>
              <select
                aria-label="任务下载线路"
                value={selected.preferredRoute || ""}
                disabled={busy}
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
                    await request("route", { id: selected.id, route, restart });
                  });
                }}
              >
                <option value="">自动选择</option>
                {routeOptions(snapshot).map((option) => (
                  <option key={option.id} value={option.id}>
                    {option.name}
                  </option>
                ))}
              </select>
            </dd>
          </>
        )}
      </dl>
      {selected.routeSuggestion && (
        <div className="route-suggestion">
          <strong>
            <Zap size={15} />
            有更快的线路
          </strong>
          <p>
            {selected.routeSuggestion.routeName} · 预计{" "}
            {duration(selected.routeSuggestion.suggestedSeconds)}，需要重下{" "}
            {bytes(selected.routeSuggestion.restartBytes)}。
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
          {(
            [
              ["up", "提前", <ArrowUp size={13} key="up" />],
              ["down", "延后", <ArrowDown size={13} key="down" />],
              ["top", "优先下载", null],
            ] as const
          ).map(([direction, label, icon]) => (
            <Button
              key={direction}
              size="sm"
              disabled={busy}
              onClick={() =>
                run(() => request("reorder", { id: selected.id, direction }))
              }
            >
              {icon}
              {label}
            </Button>
          ))}
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
          打开文件夹
        </Button>
        {finished ? (
          <Button disabled={busy} onClick={() => onRedownload(selected)}>
            <RotateCcw size={14} />
            重新下载
          </Button>
        ) : (
          <Button
            disabled={busy}
            onClick={() => run(() => request("copy", { text: selected.url }))}
          >
            <Copy size={14} />
            复制链接
          </Button>
        )}
        <Button
          variant="ghost"
          className="danger-text"
          disabled={busy || ["pausing", "cancelling"].includes(selected.status)}
          onClick={() => onAct(selected, finished ? "remove" : "cancel")}
        >
          <X size={14} />
          {finished ? "移除记录" : "取消下载"}
        </Button>
      </div>
    </aside>
  );
}
