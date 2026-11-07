import { PageHeader, usePageActiveRef } from "@/components/PageActivity";
import { useEffect, useRef, useState } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import {
  ArrowLeft,
  ArrowRight,
  AlertCircle,
  ExternalLink,
  FolderOpen,
  GitBranch,
  Link,
  List,
  Package,
  RefreshCw,
} from "lucide-react";
import { navigate, notify, request, useWorkspace } from "@/lib/api";
import {
  bytes,
  date,
  type BatchItem,
  type BatchPreview,
  type CatalogPage,
  type Snapshot,
} from "@/lib/model";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Modal } from "@/components/ui/dialog";
import { Empty, useConfirm, useJob } from "@/components/common";

const draft = {
  mode: "repository",
  repository: "",
  urls: "",
  directory: "",
  route: "",
};
export function openRepository(repository: string) {
  draft.repository = repository.startsWith("https://")
    ? repository
    : `https://github.com/${repository}`;
  draft.mode = "repository";
  window.dispatchEvent(
    new CustomEvent("githubsp:repository", { detail: draft.repository }),
  );
  navigate("new");
}
export function Markdown({ children }: { children: string }) {
  const { run } = useJob();
  return (
    <div className="markdown">
      <ReactMarkdown
        remarkPlugins={[remarkGfm]}
        components={{
          a: ({ href, children }) => (
            <button
              className="text-link"
              onClick={() => run(() => request("openLink", { url: href }))}
            >
              {children}
              <ExternalLink size={11} />
            </button>
          ),
          img: ({ alt }) => (
            <span className="muted">[图片：{alt || "未加载"}]</span>
          ),
        }}
      >
        {children}
      </ReactMarkdown>
    </div>
  );
}
function platform(name: string) {
  const n = name.toLowerCase();
  return /windows|win32|win64|\.exe$|\.msi$/.test(n)
    ? "windows"
    : /darwin|macos|\.dmg$/.test(n)
      ? "macos"
      : /linux|\.deb$|\.rpm$|\.appimage$/.test(n)
        ? "linux"
        : "other";
}

export function NewDownload() {
  const visible = usePageActiveRef();
  const { snapshot, connected } = useWorkspace();
  const [mode, setMode] = useState(draft.mode);
  const [repository, setRepository] = useState(draft.repository);
  const [urls, setUrls] = useState(draft.urls);
  const [directory, setDirectory] = useState(
    draft.directory || snapshot?.lastDirectory || "",
  );
  const [route, setRoute] = useState(draft.route);
  const [catalog, setCatalog] = useState<CatalogPage | null>(null);
  const [browsing, setBrowsing] = useState(false);
  const [browseError, setBrowseError] = useState<string | null>(null);
  const [releaseId, setRelease] = useState<number | null>(null);
  const [selected, setSelected] = useState<string[]>([]);
  const [includePrerelease, setPrerelease] = useState(false);
  const [filter, setFilter] = useState("all");
  const [notes, setNotes] = useState(false);
  const [preview, setPreview] = useState<BatchPreview | null>(null);
  const [results, setResults] = useState<BatchItem[] | null>(null);
  const epoch = useRef(0);
  const { busy, run, active } = useJob();
  const confirm = useConfirm();
  useEffect(() => {
    const open = (event: Event) => {
      setMode("repository");
      setRepository((event as CustomEvent<string>).detail);
      setCatalog(null);
      setSelected([]);
      setBrowseError(null);
      setPreview(null);
      epoch.current++;
    };
    window.addEventListener("githubsp:repository", open);
    return () => window.removeEventListener("githubsp:repository", open);
  }, []);
  const release = catalog?.releases.find((r) => r.id === releaseId);
  const eligible = (release?.assets ?? []).filter(
    (a) => !a.unavailable && (filter === "all" || platform(a.name) === filter),
  );
  const updateDirectory = (value: string) => {
    draft.directory = value;
    setDirectory(value);
    epoch.current++;
  };
  const load = (more = false) =>
    run(async () => {
      const revision = ++epoch.current;
      setBrowsing(true);
      setBrowseError(null);
      try {
        const next = await request<CatalogPage>("browse", {
          input: repository.trim(),
          page: more ? (catalog?.page ?? 0) + 1 : 1,
          includePrerelease,
        });
        if (!active.current || revision !== epoch.current) return;
        setCatalog(
          more && catalog
            ? {
                ...next,
                releases: [
                  ...catalog.releases,
                  ...next.releases.filter(
                    (r) => !catalog.releases.some((old) => old.id === r.id),
                  ),
                ],
              }
            : next,
        );
        if (!more) {
          const first = next.releases[0];
          setRelease(first?.id ?? null);
          setSelected(
            snapshot?.settings.autoMatchPlatform
              ? (first?.assets
                  .filter(
                    (a) => platform(a.name) === "windows" && !a.unavailable,
                  )
                  .slice(0, 100)
                  .map((a) => a.url) ?? [])
              : [],
          );
        }
      } catch (error) {
        if (active.current && revision === epoch.current)
          setBrowseError(
            error instanceof Error ? error.message : String(error),
          );
      } finally {
        if (active.current) setBrowsing(false);
      }
    });
  const prepare = async (revision: number) => {
    const inspected = await request<{ state: string; directory: string }>(
      "inspectDirectory",
      { directory: directory.trim() },
    );
    if (!active.current || !visible.current || revision !== epoch.current)
      return null;
    if (inspected.state === "missing") {
      if (
        !(await confirm(
          "创建保存目录？",
          `将创建以下文件夹及缺失的父目录：\n${inspected.directory}`,
          "创建目录",
        ))
      )
        return null;
      if (!active.current || !visible.current || revision !== epoch.current)
        return null;
      return request<string>("createDirectory", {
        directory: inspected.directory,
      });
    }
    return inspected.directory;
  };
  const showPreview = () =>
    run(async () => {
      const revision = ++epoch.current;
      const target = await prepare(revision);
      if (
        !target ||
        !active.current ||
        !visible.current ||
        revision !== epoch.current
      )
        return;
      const next = await request<BatchPreview>("preview", {
        urls:
          mode === "repository"
            ? selected
            : urls
                .split(/\r?\n/)
                .map((v) => v.trim())
                .filter(Boolean),
        directory: target,
      });
      if (!active.current || !visible.current || revision !== epoch.current)
        return;
      setPreview(next);
      setResults(null);
    });
  const valid =
    preview?.items.filter((item) => item.status === "valid" && item.url) ?? [];
  return (
    <section className="page new-page" tabIndex={-1}>
      <PageHeader>
        <div>
          <h1>新建下载</h1>
        </div>
        <Button variant="ghost" onClick={() => navigate("queue")}>
          <ArrowLeft size={15} />
          返回队列
        </Button>
      </PageHeader>
      <div className="new-content">
        <div className="source-types" role="group" aria-label="下载来源">
          {[
            ["repository", "仓库附件", GitBranch],
            ["direct", "附件直链", Link],
            ["batch", "批量链接", List],
          ].map(([key, label, Icon]) => (
            <button
              key={String(key)}
              aria-pressed={mode === key}
              disabled={busy}
              className={mode === key ? "selected" : ""}
              onClick={() => {
                setMode(String(key));
                draft.mode = String(key);
                setBrowseError(null);
                epoch.current++;
              }}
            >
              <Icon size={16} />
              <span>{String(label)}</span>
            </button>
          ))}
        </div>
        <div className="source-input">
          <label className="field-label" htmlFor="source-url">
            {mode === "repository"
              ? "GitHub 仓库或 Release 地址"
              : mode === "batch"
                ? "附件下载链接，每行一条"
                : "GitHub Release 附件直链"}
          </label>
          <div className="input-row">
            {mode === "repository" ? (
              <>
                <Input
                  id="source-url"
                  placeholder="https://github.com/owner/repository"
                  value={repository}
                  disabled={busy}
                  onChange={(e) => {
                    draft.repository = e.target.value;
                    setRepository(e.target.value);
                    epoch.current++;
                    setCatalog(null);
                    setBrowseError(null);
                    setSelected([]);
                  }}
                  onKeyDown={(e) => {
                    if (
                      e.key === "Enter" &&
                      !e.nativeEvent.isComposing &&
                      repository.trim()
                    )
                      load();
                  }}
                />
                <Button
                  variant="default"
                  disabled={busy || !connected || !repository.trim()}
                  onClick={() => load()}
                >
                  <RefreshCw size={15} className={browsing ? "spin" : ""} />
                  {browsing
                    ? "正在读取…"
                    : browseError
                      ? "重新读取"
                      : "读取版本"}
                </Button>
              </>
            ) : (
              <textarea
                id="source-url"
                className="input url-textarea"
                rows={mode === "batch" ? 5 : 3}
                placeholder="https://github.com/owner/repo/releases/download/v1.0/file.zip"
                value={urls}
                disabled={busy}
                onChange={(e) => {
                  draft.urls = e.target.value;
                  setUrls(e.target.value);
                  epoch.current++;
                }}
              />
            )}
          </div>
          {mode === "repository" && (
            <label className="checkbox-line">
              <input
                type="checkbox"
                checked={includePrerelease}
                disabled={busy}
                onChange={(e) => {
                  setPrerelease(e.target.checked);
                  setCatalog(null);
                  setBrowseError(null);
                  setSelected([]);
                  epoch.current++;
                }}
              />
              显示预发布版本<span>更改后重新读取</span>
            </label>
          )}
          {browseError && (
            <div className="source-error" role="alert">
              <AlertCircle size={16} />
              <span>{browseError}</span>
            </div>
          )}
        </div>
        {mode === "repository" ? (
          catalog ? (
            <section
              className="release-browser"
              aria-label="版本与附件"
              aria-busy={browsing}
            >
              <header className="catalog-heading">
                <div className="catalog-repository">
                  <GitBranch size={18} />
                  <strong title={catalog.repository}>
                    {catalog.repository}
                  </strong>
                </div>
                <div className="catalog-version">
                  <label className="sr-only" htmlFor="release-version">
                    发布版本
                  </label>
                  <select
                    id="release-version"
                    value={releaseId ?? ""}
                    disabled={busy || !catalog.releases.length}
                    onChange={(event) => setRelease(Number(event.target.value))}
                  >
                    {!catalog.releases.length && (
                      <option value="">暂无发布版本</option>
                    )}
                    {catalog.releases.map((item) => (
                      <option key={item.id} value={item.id}>
                        {item.tag}
                        {item.prerelease ? " · 预发布" : ""}
                      </option>
                    ))}
                  </select>
                  {catalog.hasMore && (
                    <Button
                      size="sm"
                      variant="ghost"
                      disabled={busy}
                      onClick={() => load(true)}
                    >
                      更多版本
                    </Button>
                  )}
                </div>
              </header>
              <div className="asset-pane">
                {release ? (
                  <>
                    <div className="asset-heading">
                      <div>
                        <h2>{release.name || release.tag}</h2>
                        <p>
                          {date(release.publishedAt).split(" ")[0]} ·{" "}
                          {release.assets.length} 个附件
                        </p>
                      </div>
                      <Button
                        size="sm"
                        variant="ghost"
                        onClick={() => setNotes(!notes)}
                      >
                        {notes ? "收起说明" : "更新说明"}
                      </Button>
                    </div>
                    {notes && (
                      <div className="release-notes">
                        <Markdown>
                          {release.notes || "此版本没有提供更新说明。"}
                        </Markdown>
                      </div>
                    )}
                    <div className="asset-toolbar">
                      <select
                        aria-label="附件平台筛选"
                        value={filter}
                        onChange={(e) => setFilter(e.target.value)}
                      >
                        <option value="all">全部平台</option>
                        <option value="windows">Windows</option>
                        <option value="macos">macOS</option>
                        <option value="linux">Linux</option>
                        <option value="other">其他附件</option>
                      </select>
                      <Button
                        size="sm"
                        variant="ghost"
                        onClick={() =>
                          setSelected(
                            [
                              ...new Set([
                                ...selected,
                                ...eligible.map((a) => a.url),
                              ]),
                            ].slice(0, 100),
                          )
                        }
                      >
                        全选当前列表
                      </Button>
                      <Button
                        size="sm"
                        variant="ghost"
                        onClick={() => setSelected([])}
                      >
                        清空选择
                      </Button>
                    </div>
                    <div className="asset-list" aria-label="发布附件">
                      {!release.assets.some(
                        (asset) =>
                          filter === "all" || platform(asset.name) === filter,
                      ) && (
                        <p className="asset-no-match">
                          当前筛选下没有附件，试试其他平台。
                        </p>
                      )}
                      {release.assets
                        .filter(
                          (a) =>
                            filter === "all" || platform(a.name) === filter,
                        )
                        .map((asset) => (
                          <label
                            className={`asset-row ${asset.unavailable ? "unavailable" : ""}`}
                            key={asset.id}
                          >
                            <input
                              type="checkbox"
                              checked={selected.includes(asset.url)}
                              disabled={
                                !!asset.unavailable ||
                                busy ||
                                (!selected.includes(asset.url) &&
                                  selected.length >= 100)
                              }
                              onChange={(e) =>
                                setSelected(
                                  e.target.checked
                                    ? [...selected, asset.url]
                                    : selected.filter((v) => v !== asset.url),
                                )
                              }
                            />
                            <Package size={19} />
                            <div>
                              <strong>{asset.name}</strong>
                              <small>
                                {asset.unavailable ||
                                  asset.hints.join(" · ") ||
                                  "请确认平台和架构"}
                              </small>
                            </div>
                            <span>{bytes(asset.size)}</span>
                          </label>
                        ))}
                    </div>
                  </>
                ) : (
                  <Empty
                    title="这个仓库还没有发布版本"
                    description="可以更换仓库地址，或使用附件直链下载。"
                  />
                )}
              </div>
            </section>
          ) : browsing ? (
            <div
              className="catalog-loading"
              role="status"
              aria-label="正在读取 GitHub 版本"
            >
              <p>
                <RefreshCw size={15} className="spin" />
                正在读取版本与附件…
              </p>
              <div aria-hidden="true">
                {[0, 1, 2, 3].map((index) => (
                  <div className="catalog-skeleton-row" key={index}>
                    <span />
                    <div>
                      <i />
                      <i />
                    </div>
                    <span />
                  </div>
                ))}
              </div>
            </div>
          ) : (
            <div className="source-start">
              <GitBranch size={24} strokeWidth={1.5} />
              <div>
                <h2>从一个 GitHub 项目开始</h2>
                <p>
                  读取仓库后，在这里选择发布版本和附件。也支持直接粘贴 Release
                  页面地址。
                </p>
              </div>
            </div>
          )
        ) : (
          <div className="direct-explainer">
            <Link size={24} />
            <div>
              <h2>
                {mode === "batch" ? "一次确认，依次下载" : "直达你需要的附件"}
              </h2>
              <p>
                支持公开 GitHub Release
                附件。提交前会检查保存目录并识别重复链接；大小未知时会在开始下载后更新。每批最多
                100 项。
              </p>
            </div>
          </div>
        )}
      </div>
      <footer className="new-footer">
        <div className="download-options">
          <div>
            <label className="field-label" htmlFor="save-directory">
              保存到
            </label>
            <div className="input-row">
              <Input
                id="save-directory"
                placeholder="选择或输入保存目录"
                title={directory || "选择或输入保存目录"}
                value={directory}
                disabled={busy}
                onChange={(e) => updateDirectory(e.target.value)}
              />
              <Button
                aria-label="浏览保存目录"
                disabled={busy || !connected}
                onClick={() =>
                  run(async () => {
                    const revision = ++epoch.current;
                    const target = await request<string | null>(
                      "chooseDirectory",
                      { initial: directory || null },
                    );
                    if (target && active.current && revision === epoch.current)
                      updateDirectory(target);
                  })
                }
              >
                <FolderOpen size={16} />
              </Button>
            </div>
          </div>
          <div>
            <label className="field-label" htmlFor="download-route">
              下载线路
            </label>
            <select
              id="download-route"
              disabled={busy}
              value={route}
              onChange={(e) => {
                setRoute(e.target.value);
                draft.route = e.target.value;
              }}
            >
              <option value="">自动检测最快线路</option>
              <option value="github">GitHub 直连</option>
              <option value="gh-proxy">GH-Proxy</option>
              <option value="ghproxy-net">ghproxy.net</option>
            </select>
          </div>
        </div>
        <div className="download-submit">
          <div aria-live="polite" aria-atomic="true">
            <strong>
              {mode === "repository"
                ? selected.length
                : urls.split(/\r?\n/).filter((v) => v.trim()).length}
            </strong>{" "}
            个附件待确认
          </div>
          <Button
            variant="default"
            disabled={
              busy ||
              !connected ||
              !directory.trim() ||
              (mode === "repository" ? !selected.length : !urls.trim())
            }
            onClick={showPreview}
          >
            {busy && !browsing ? "正在准备…" : "预览并确认"}
            <ArrowRight size={15} />
          </Button>
        </div>
      </footer>
      <Modal
        open={!!preview}
        onClose={() => {
          if (!busy) {
            setPreview(null);
            setResults(null);
          }
        }}
        busy={busy}
        wide
        title="确认下载任务"
        description="核对附件与保存位置，创建后即可在队列中暂停或取消。"
        footer={
          <>
            <span className="footer-summary">
              {valid.length} 个有效附件 · {bytes(preview?.knownSize)}
              {preview?.unknownCount
                ? ` + ${preview.unknownCount} 个大小未知`
                : ""}
            </span>
            <Button disabled={busy} onClick={() => setPreview(null)}>
              返回修改
            </Button>
            <Button
              variant="default"
              disabled={busy || !valid.length || !!results}
              onClick={() =>
                run(async () => {
                  const result = await request<{
                    items: BatchItem[];
                    snapshot: Snapshot;
                  }>("create", {
                    urls: valid.map((item) => item.url),
                    directory: preview!.preflight.directory,
                    route: route || null,
                  });
                  if (!active.current) return;
                  if (result.items.every((item) => item.status === "created")) {
                    setPreview(null);
                    notify(
                      `已创建 ${result.items.length} 个下载任务`,
                      "success",
                    );
                    if (visible.current) navigate("queue");
                  } else {
                    const failed = result.items.filter(
                      (item) => item.status !== "created",
                    );
                    setResults(result.items);
                    setSelected(failed.map((item) => item.input));
                    setUrls(failed.map((item) => item.input).join("\n"));
                    draft.urls = failed.map((item) => item.input).join("\n");
                    notify("部分任务未创建，请返回修改后重试。", "error");
                  }
                })
              }
            >
              {busy ? "正在创建…" : `创建 ${valid.length} 个任务`}
            </Button>
          </>
        }
      >
        <div className="preview-directory">
          <FolderOpen size={16} />
          {preview?.preflight.directory}
        </div>
        <div className="preview-list">
          {(results || preview?.items || []).map((item, index) => (
            <div className="preview-row" key={index}>
              <span className={`status status-${item.status}`}>
                {{
                  valid: "有效",
                  invalid: "无效",
                  duplicate: "重复",
                  created: "已创建",
                  failed: "失败",
                }[item.status] || item.status}
              </span>
              <div>
                <strong>{item.filename || item.input}</strong>
                {item.message && <small>{item.message}</small>}
              </div>
              {item.taskId && item.status === "duplicate" && (
                <Button
                  size="sm"
                  onClick={() =>
                    run(() =>
                      request("action", { id: item.taskId, action: "resume" }),
                    )
                  }
                >
                  继续原任务
                </Button>
              )}
            </div>
          ))}
        </div>
        {!!preview?.unknownCount && (
          <p className="form-note">
            部分附件大小未知，无法完整估算磁盘空间；实际空间不足时下载会停止并保留错误信息。
          </p>
        )}
      </Modal>
    </section>
  );
}
