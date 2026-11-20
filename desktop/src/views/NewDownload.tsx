import { PageHeader, usePageActiveRef } from "@/components/PageActivity";
import { AssetPreferenceFields } from "@/components/AssetPreferenceFields";
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
  Package,
  RefreshCw,
} from "lucide-react";
import { navigate, notify, request, useWorkspace } from "@/lib/api";
import { detectSource } from "@/lib/source";
import { pasteSource } from "@/lib/pasteSource";
import {
  bytes,
  date,
  defaultPreference,
  type AssetPreference,
  type Recommendation,
  type BatchItem,
  type BatchPreview,
  type CatalogPage,
  routeOptions,
  type Snapshot,
} from "@/lib/model";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Modal } from "@/components/ui/dialog";
import { Empty, useConfirm, useJob } from "@/components/common";

const draft = {
  source: "",
  directory: "",
  route: "",
  autoload: false,
};
/** 新建下载输入框里当前的内容，剪贴板提示据此跳过已经填过的链接。 */
export const draftSource = () => draft.source.trim();
/** 打开新建下载并填入地址；仓库地址默认直接读取版本。 */
export function openRepository(repository: string, autoload = true) {
  draft.source = repository.startsWith("https://")
    ? repository
    : `https://github.com/${repository}`;
  draft.autoload = autoload;
  window.dispatchEvent(
    new CustomEvent("githubsp:repository", { detail: draft.source }),
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
  const [source, setSource] = useState(draft.source);
  const [autoload, setAutoload] = useState(draft.autoload);
  const detected = detectSource(source);
  const mode = detected.kind === "links" ? "direct" : "repository";
  const repository = detected.kind === "repository" ? detected.lines[0] : "";
  const urls = detected.kind === "links" ? detected.lines.join("\n") : "";
  const changeSource = (value: string) => {
    draft.source = value;
    setSource(value);
    epoch.current++;
    browseEpoch.current++;
    setCatalog(null);
    setBrowseError(null);
    setSelected([]);
    setPreview(null);
  };
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
  const [preference, setPreference] = useState<AssetPreference>(defaultPreference);
  const [recommendation, setRecommendation] = useState<Recommendation | null>(null);
  const [selectionError, setSelectionError] = useState<string | null>(null);
  const selectionTouched = useRef(false);
  const [preview, setPreview] = useState<BatchPreview | null>(null);
  const [results, setResults] = useState<BatchItem[] | null>(null);
  const epoch = useRef(0);
  // 读取版本单独计数：切换附件选择会让 epoch 增加，但不应作废正在进行的读取。
  const browseEpoch = useRef(0);
  const { busy, run, active } = useJob();
  const confirm = useConfirm();
  useEffect(() => {
    const open = (event: Event) => {
      setSource((event as CustomEvent<string>).detail);
      setAutoload(draft.autoload);
      setCatalog(null);
      setSelected([]);
      setBrowseError(null);
      setPreview(null);
      epoch.current++;
      browseEpoch.current++;
    };
    window.addEventListener("githubsp:repository", open);
    return () => window.removeEventListener("githubsp:repository", open);
  }, []);
  useEffect(() => {
    if (!autoload || !connected) return;
    setAutoload(false);
    draft.autoload = false;
    if (detected.kind === "repository") load();
  }, [autoload, connected, source]);
  const release = catalog?.releases.find((r) => r.id === releaseId);
  const projectRule = catalog
    ? snapshot?.watchState.repositories[catalog.repository.toLowerCase()]?.rule
    : undefined;
  const pattern = projectRule?.assetPattern || snapshot?.settings.watchAssetPattern || null;
  useEffect(() => {
    epoch.current++;
    setPreview(null);
    setSelected([]);
    setRecommendation(null);
    setSelectionError(null);
    selectionTouched.current = false;
    if (!release) return;
    let cancelled = false;
    request<Recommendation>("selectAssets", { release, preference, pattern })
      .then(result => {
        if (cancelled) return;
        setRecommendation(result);
        if (snapshot?.settings.autoMatchPlatform && !selectionTouched.current)
          setSelected(result.selectedUrls);
      })
      .catch(error => { if (!cancelled) setSelectionError(String(error)); });
    return () => { cancelled = true; };
  }, [release, preference, pattern, snapshot?.settings.autoMatchPlatform]);
  const selectManually = (urls: string[]) => {
    selectionTouched.current = true;
    epoch.current++;
    setPreview(null);
    setSelected(urls);
  };
  const eligible = (release?.assets ?? []).filter(
    (a) => !a.unavailable && (filter === "all" || platform(a.name) === filter),
  );
  const updateDirectory = (value: string) => {
    draft.directory = value;
    setDirectory(value);
    epoch.current++;
  };
  const load = (more = false, prerelease = includePrerelease) =>
    run(async () => {
      const revision = ++browseEpoch.current;
      epoch.current++;
      setBrowsing(true);
      setBrowseError(null);
      try {
        const next = await request<CatalogPage>("browse", {
          input: repository.trim(),
          page: more ? (catalog?.page ?? 0) + 1 : 1,
          includePrerelease: prerelease,
        });
        if (!active.current || revision !== browseEpoch.current) return;
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
          setPreference(snapshot?.watchState.repositories[next.repository.toLowerCase()]?.rule.preference ?? defaultPreference);
          setSelected([]);
        }
      } catch (error) {
        if (active.current && revision === browseEpoch.current)
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
        // 版本列表里已有附件大小，一并交给预览，省去再次查询。
        sizes:
          mode === "repository"
            ? Object.fromEntries(
                (release?.assets ?? [])
                  .filter((asset) => selected.includes(asset.url))
                  .map((asset) => [asset.url, asset.size]),
              )
            : {},
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
        <div className="source-input">
          <label className="field-label" htmlFor="source-url">
            粘贴 GitHub 地址
            <span className="muted">
              仓库或 Release 页面地址，或者一个、多个附件链接（每行一个）
            </span>
          </label>
          <div className="input-row">
            <textarea
              id="source-url"
              className="input source-field"
              rows={Math.min(Math.max(detected.lines.length, 1), 6)}
              placeholder="https://github.com/owner/repository"
              value={source}
              disabled={busy}
              spellCheck={false}
              onChange={(e) => changeSource(e.target.value)}
              onPaste={(e) => pasteSource(e, changeSource)}
              onKeyDown={(e) => {
                // 仓库地址按回车直接读取版本；附件链接仍可换行继续粘贴。
                if (
                  e.key === "Enter" &&
                  !e.shiftKey &&
                  !e.nativeEvent.isComposing &&
                  detected.kind === "repository"
                ) {
                  e.preventDefault();
                  load();
                }
              }}
            />
            {detected.kind !== "links" && (
              <label className="checkbox-line prerelease-toggle">
                <input
                  type="checkbox"
                  checked={includePrerelease}
                  disabled={busy}
                  onChange={(e) => {
                    const next = e.target.checked;
                    setPrerelease(next);
                    // 已读取过版本时自动按新选项重读；还没读取时只记住选项。
                    if (
                      (catalog || browseError) &&
                      detected.kind === "repository" &&
                      connected
                    )
                      load(false, next);
                  }}
                />
                显示预发布版本
              </label>
            )}
            {detected.kind !== "links" && (
              <Button
                variant="default"
                disabled={
                  busy || !connected || detected.kind !== "repository"
                }
                onClick={() => load()}
              >
                <RefreshCw size={15} className={browsing ? "spin" : ""} />
                {browsing
                  ? "正在读取…"
                  : browseError
                    ? "重新读取"
                    : "读取版本"}
              </Button>
            )}
          </div>
          {detected.kind === "links" && (
            <p className="source-hint" role="status">
              <Link size={13} />
              识别到 {detected.lines.length} 个附件链接
              {detected.lines.length > 100 && "，每次最多 100 个"}
              。预览时会列出文件大小并提醒重复下载。
            </p>
          )}
          {detected.kind === "mixed" && (
            <p className="source-hint is-warning" role="status">
              <AlertCircle size={13} />
              多行时每行都要是附件链接；仓库地址请单独粘贴。
            </p>
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
                    <div className="asset-preference-panel">
                      <AssetPreferenceFields value={preference} onChange={setPreference} disabled={busy} />
                      <div className="asset-preference-actions">
                        <p role="status">{selectionError || recommendation?.message || "正在匹配附件…"}</p>
                        <Button size="sm" variant="ghost" disabled={busy || !connected}
                          onClick={() => run(async () => {
                            const repository = catalog.repository;
                            if (!snapshot?.favorites.some(favorite => favorite.repository.toLowerCase() === repository.toLowerCase()))
                              await request("favoriteAdd", { input: `https://github.com/${repository}` });
                            await request("watchRule", { repository, rule: {
                              includePrerelease: projectRule?.includePrerelease ?? false,
                              autoDownload: projectRule?.autoDownload ?? false,
                              assetPattern: projectRule?.assetPattern ?? null,
                              preference,
                            } });
                            notify("项目下载偏好已保存", "success");
                          })}>
                          {snapshot?.favorites.some(favorite => favorite.repository.toLowerCase() === catalog.repository.toLowerCase()) ? "保存项目偏好" : "关注并保存偏好"}
                        </Button>
                      </div>
                      {pattern && <p className="form-note">同时匹配名称规则：{pattern}</p>}
                    </div>
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
                          selectManually(
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
                        onClick={() => selectManually([])}
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
                                selectManually(
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
                                  recommendation?.candidates.find(candidate => candidate.url === asset.url)?.reason ||
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
                    description="换一个仓库地址，或者直接粘贴附件链接。"
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
          ) : null
        ) : null}
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
              <option value="">自动选择最快线路</option>
              {routeOptions(snapshot).map((option) => (
                <option key={option.id} value={option.id}>
                  {option.name}
                </option>
              ))}
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
        description="核对文件和保存位置。加入队列后也可以随时暂停或取消。"
        footer={
          <>
            <span className="footer-summary">
              {valid.length} 个文件
              {preview && preview.unknownCount < valid.length &&
                ` · 共 ${bytes(preview.knownSize)}`}
              {!!preview?.unknownCount &&
                (preview.unknownCount === valid.length
                  ? " · 大小在下载时确定"
                  : ` · 另有 ${preview.unknownCount} 个大小待定`)}
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
                    changeSource(failed.map((item) => item.input).join("\n"));
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
              {item.status !== "invalid" && (
                <span className="preview-size">
                  {item.size == null ? "大小待定" : bytes(item.size)}
                </span>
              )}
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
            有些文件暂时查不到大小，开始下载后会显示。磁盘空间是否够用也要等那时才能确认。
          </p>
        )}
      </Modal>
    </section>
  );
}
