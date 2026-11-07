import { PageHeader, usePageActive } from "@/components/PageActivity";
import { useEffect, useState } from "react";
import {
  ArrowLeft,
  ArrowRight,
  Copy,
  FolderOpen,
  RefreshCw,
  Search,
  Trash2,
} from "lucide-react";
import { notify, request, useWorkspace } from "@/lib/api";
import {
  bytes,
  date,
  statusLabels,
  terminal,
  type HistoryPage,
} from "@/lib/model";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Busy, Empty, ErrorBox, useConfirm, useJob } from "@/components/common";

export function History() {
  const visible = usePageActive();
  const { snapshot, connected } = useWorkspace();
  const [query, setQuery] = useState("");
  const [status, setStatus] = useState("");
  const [page, setPage] = useState(1);
  const [result, setResult] = useState<HistoryPage | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");
  const [revision, refresh] = useState(0);
  const { busy, run } = useJob();
  const confirm = useConfirm();
  useEffect(() => {
    if (!connected || !visible) return;
    let cancelled = false;
    setLoading(true);
    const timer = setTimeout(() => {
      request<HistoryPage>("history", { query, status: status || null, page })
        .then((value) => {
          if (!cancelled) {
            setResult(value);
            setError("");
          }
        })
        .catch((error) => {
          if (!cancelled) setError(String(error));
        })
        .finally(() => {
          if (!cancelled) setLoading(false);
        });
    }, 180);
    return () => {
      cancelled = true;
      clearTimeout(timer);
    };
  }, [
    query,
    status,
    page,
    revision,
    snapshot?.historyRevision,
    connected,
    visible,
  ]);
  return (
    <section className="page history-page" tabIndex={-1}>
      <PageHeader>
        <div>
          <h1>下载历史</h1>
        </div>
        <Button
          disabled={busy || !connected}
          onClick={() => refresh((v) => v + 1)}
        >
          <RefreshCw size={15} />
          刷新记录
        </Button>
      </PageHeader>
      <div className="panel history-panel">
        <div className="list-toolbar">
          <label className="search-box grow">
            <Search size={16} />
            <Input
              aria-label="搜索历史记录"
              placeholder="搜索文件名、仓库或版本"
              value={query}
              onChange={(e) => {
                setQuery(e.target.value);
                setPage(1);
              }}
            />
          </label>
          <select
            aria-label="历史状态筛选"
            value={status}
            onChange={(e) => {
              setStatus(e.target.value);
              setPage(1);
            }}
          >
            <option value="">全部状态</option>
            <option value="completed">已完成</option>
            <option value="cancelled">已取消</option>
            <option value="failed">失败</option>
            <option value="paused">已暂停</option>
          </select>
        </div>
        {error && <ErrorBox text={error} />}
        {loading && !result && <Busy />}
        <div className="history-list">
          {result?.items.length
            ? result.items.map(({ task, fileState }) => (
                <article className="history-row" key={task.id}>
                  <div className="file-icon">
                    <FolderOpen size={18} />
                  </div>
                  <div className="history-row-name">
                    <strong title={`${task.filename}\n${task.directory}`}>
                      {task.filename}
                    </strong>
                    <small>
                      {task.repository || "GitHub Release"} ·{" "}
                      {task.tag || "附件"} ·{" "}
                      {date(task.completedAt || task.createdAt)}
                    </small>
                  </div>
                  <div className="history-row-status">
                    <span className={`status status-${task.status}`}>
                      {statusLabels[task.status]}
                    </span>
                    <small>
                      {bytes(task.total)} ·{" "}
                      {fileState === "missing"
                        ? "文件已缺失"
                        : fileState === "unavailable"
                          ? "文件不可访问"
                          : task.verification === "verified"
                            ? "校验通过"
                            : "本地记录"}
                    </small>
                  </div>
                  <div className="row-actions">
                    <Button
                      size="icon"
                      variant="ghost"
                      aria-label={`打开 ${task.filename} 所在目录`}
                      disabled={busy}
                      onClick={() =>
                        run(() => request("openDirectory", { taskId: task.id }))
                      }
                    >
                      <FolderOpen size={16} />
                    </Button>
                    <Button
                      size="icon"
                      variant="ghost"
                      aria-label={`复制 ${task.filename} 链接`}
                      disabled={busy}
                      onClick={() =>
                        run(() => request("copy", { text: task.url }))
                      }
                    >
                      <Copy size={15} />
                    </Button>
                    <Button
                      size="sm"
                      disabled={busy}
                      onClick={() =>
                        run(async () => {
                          const result = await request<{
                            items: { status: string; message: string | null }[];
                          }>("create", {
                            urls: [task.url],
                            directory: task.directory,
                            route: null,
                          });
                          const failure = result.items.find(
                            (item) => item.status !== "created",
                          );
                          if (failure)
                            throw new Error(
                              failure.message ||
                                "任务未创建，请检查链接与保存目录",
                            );
                          notify("已加入下载队列", "success");
                          refresh((v) => v + 1);
                        })
                      }
                    >
                      重新下载
                    </Button>
                    {terminal(task.status) && (
                      <Button
                        size="icon"
                        variant="ghost"
                        aria-label={`移除 ${task.filename} 记录`}
                        disabled={busy}
                        onClick={() =>
                          run(async () => {
                            if (
                              await confirm(
                                "移除这条历史记录？",
                                "已经下载的文件会保留。",
                                "移除记录",
                              )
                            ) {
                              await request("action", {
                                id: task.id,
                                action: "remove",
                              });
                              refresh((v) => v + 1);
                            }
                          })
                        }
                      >
                        <Trash2 size={15} />
                      </Button>
                    )}
                  </div>
                </article>
              ))
            : !loading && (
                <Empty
                  title="还没有历史记录"
                  description="下载完成后，可以在这里查找、重新下载或打开文件目录。"
                />
              )}
        </div>
        <footer className="pagination">
          <span>共 {result?.total ?? 0} 条 · 每页 20 条</span>
          <div>
            <Button
              size="sm"
              disabled={loading || page === 1}
              onClick={() => setPage((v) => v - 1)}
            >
              <ArrowLeft size={13} />
              上一页
            </Button>
            <span>
              第 {page} / {Math.max(1, Math.ceil((result?.total ?? 0) / 20))} 页
            </span>
            <Button
              size="sm"
              disabled={loading || !result || page * 20 >= result.total}
              onClick={() => setPage((v) => v + 1)}
            >
              下一页
              <ArrowRight size={13} />
            </Button>
          </div>
        </footer>
      </div>
    </section>
  );
}
