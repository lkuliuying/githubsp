import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import {
  ArrowDownToLine,
  ArrowRight,
  BellPlus,
  CheckCheck,
  EyeOff,
  ExternalLink,
  FolderOpen,
  ListChecks,
  Package,
  Plus,
  RefreshCw,
  Search,
  Settings2,
  Trash2,
  X,
} from "lucide-react";
import { PageHeader, usePageActive } from "@/components/PageActivity";
import { AssetPreferenceFields } from "@/components/AssetPreferenceFields";
import { WatchlistTransfer } from "@/components/WatchlistTransfer";
import { Menu } from "@/components/Menu";
import { Pager, clampPage, usePageSize } from "@/components/Pager";
import { Empty, ErrorBox, Toggle, useConfirm, useJob } from "@/components/common";
import { Button } from "@/components/ui/button";
import { CopyButton } from "@/components/CopyButton";
import { useSpringThumb } from "@/lib/useSpringThumb";
import { RollingNumber } from "@/components/RollingNumber";
import { Input } from "@/components/ui/input";
import { Modal } from "@/components/ui/dialog";
import { navigate, notify, request, useWorkspace } from "@/lib/api";
import {
  bytes,
  date,
  defaultPreference,
  fileStateLabels,
  relativeDay,
  type LocalSoftware,
  type UpdateEvent,
  type UpdatePage,
  type WatchRule,
} from "@/lib/model";
import {
  buildProjects,
  stateLabels,
  type SoftwareProject,
} from "@/lib/software";
import { Markdown, openRepository } from "./NewDownload";
import { UpdateFeed } from "./Updates";
import { pasteSource } from "@/lib/pasteSource";

const filters = [
  ["all", "全部"],
  ["update", "可更新"],
  ["watched", "已关注"],
  ["unwatched", "未关注"],
  ["issue", "文件异常"],
] as const;
type Filter = (typeof filters)[number][0];
const verificationLabels: Record<string, string> = {
  verified: "官方 SHA-256 匹配",
  unverified: "没有官方摘要",
  pending: "等待校验",
  failed: "校验失败",
};

export function MySoftware() {
  const visible = usePageActive();
  const { snapshot, connected } = useWorkspace();
  const [tab, setTab] = useState<"software" | "feed">("software");
  const [filter, setFilter] = useState<Filter>("all");
  const viewThumb = useSpringThumb(tab);
  const filterThumb = useSpringThumb(filter);
  const [group, setGroup] = useState("");
  const [query, setQuery] = useState("");
  const [page, setPage] = useState(1);
  const [pageSize, setPageSize] = usePageSize("software");
  const [locals, setLocals] = useState<LocalSoftware[] | null>(null);
  const [error, setError] = useState("");
  const [revision, refresh] = useState(0);
  const [selection, select] = useState<string | null>(null);
  const [adding, setAdding] = useState(false);
  const [input, setInput] = useState("");
  const [editing, setEditing] = useState<SoftwareProject | null>(null);
  const detailTrigger = useRef<HTMLButtonElement | null>(null);
  const { busy, run, active } = useJob();
  const confirm = useConfirm();

  useEffect(() => {
    if (!visible || !connected) return;
    let cancelled = false;
    request<LocalSoftware[]>("softwareProjects")
      .then((value) => {
        if (!cancelled) {
          setLocals(value);
          setError("");
        }
      })
      .catch((reason) => {
        if (!cancelled) setError(String(reason));
      });
    return () => {
      cancelled = true;
    };
  }, [visible, connected, snapshot?.historyRevision, revision]);

  const projects = useMemo(
    () => (snapshot ? buildProjects(snapshot, locals ?? []) : []),
    [snapshot, locals],
  );
  const groups = [
    ...new Set(
      projects
        .map((project) => project.entry?.group)
        .filter((value): value is string => !!value),
    ),
  ];
  const matches = (project: SoftwareProject, key: Filter) =>
    key === "all" ||
    (key === "update" && project.state === "update") ||
    (key === "watched" && !!project.favorite) ||
    (key === "unwatched" && !project.favorite) ||
    (key === "issue" && project.fileIssue);
  const shown = projects.filter(
    (project) =>
      matches(project, filter) &&
      (!group || project.entry?.group === group) &&
      `${project.repository} ${project.entry?.group ?? ""} ${project.entry?.tags?.join(" ") ?? ""} ${project.local?.filename ?? ""}`
        .toLowerCase()
        .includes(query.trim().toLowerCase()),
  );
  const listPage = clampPage(page, shown.length, pageSize);
  const pagedProjects = shown.slice((listPage - 1) * pageSize, listPage * pageSize);
  const selected = projects.find((project) => project.key === selection);
  const updates = projects.filter((project) => project.state === "update").length;
  const issues = projects.filter((project) => project.fileIssue).length;
  const unreadCount = projects.filter((project) => project.unread).length;
  const closeDetails = () => {
    select(null);
    detailTrigger.current?.focus({ preventScroll: true });
  };

  // ---- 操作 ----
  const download = (project: SoftwareProject) =>
    run(async () => {
      try {
        await request("favoriteDownload", { repository: project.repository });
        notify(`${project.repository} 已加入下载队列`, "success");
      } catch (reason) {
        const message = reason instanceof Error ? reason.message : String(reason);
        if (
          await confirm(
            "没能自动选出附件",
            `${message}\n\n可以打开版本列表手动选择要下载的附件。`,
            "手动选择",
          )
        )
          openRepository(project.repository);
      }
    });
  // 依次把所有可更新的项目加入队列；选不出附件的项目汇总提示，不逐个弹窗。
  const updateAll = () =>
    run(async () => {
      const targets = projects.filter((project) => project.state === "update");
      const failed: string[] = [];
      for (const project of targets) {
        try {
          await request("favoriteDownload", { repository: project.repository });
        } catch {
          failed.push(project.repository);
        }
      }
      const added = targets.length - failed.length;
      if (!failed.length) notify(`已把 ${added} 个更新加入下载队列`, "success");
      else
        notify(
          `${added ? `已加入 ${added} 个更新；` : ""}${failed.join("、")} 没能自动选出附件，请在这一行点「更新到」手动选择`,
          added ? "info" : "error",
        );
    });
  const watch = (project: SoftwareProject) =>
    run(async () => {
      await request("favoriteAdd", {
        input: `https://github.com/${project.repository}`,
      });
      notify(`已关注 ${project.repository}，有新版本时会提醒你`, "success");
    });
  const unwatch = (project: SoftwareProject) =>
    run(async () => {
      if (
        await confirm(
          "取消关注这个项目？",
          `${project.repository} 不再检查新版本。已下载的文件不受影响。`,
          "取消关注",
        )
      )
        await request("favoriteRemove", { repository: project.repository });
    });
  const openFolder = (project: SoftwareProject) =>
    project.local &&
    run(() => request("openDirectory", { taskId: project.local!.taskId }));
  const check = (project?: SoftwareProject) =>
    run(() =>
      request("favoriteCheck", { repository: project?.repository ?? null }),
    );
  const dismiss = (project: SoftwareProject) =>
    run(() => request("favoriteDismiss", { repository: project.repository }));
  const skip = (project: SoftwareProject) =>
    run(async () => {
      const latest = project.favorite?.latest;
      if (!latest) return;
      // 「忽略此版本」记录在更新动态里；基线版本没有动态时退化为标为已读。
      const page = await request<UpdatePage>("updates", {
        cursor: null,
        limit: 20,
        repository: project.repository,
        recent: true,
      });
      const event = page.events.find((item) => item.release.id === latest.id);
      if (event)
        await request("updateAction", {
          id: event.id,
          action: project.skipped ? "unskip" : "skip",
        });
      else await request("favoriteDismiss", { repository: project.repository });
    });

  const primary = (project: SoftwareProject, wide = false) => {
    const tag = project.favorite?.latest?.tag;
    const className = wide ? "sw-primary-wide" : undefined;
    if (project.state === "update")
      return (
        <Button
          size={wide ? "default" : "sm"}
          variant="default"
          className={className}
          disabled={busy || !connected}
          onClick={() => download(project)}
        >
          <ArrowDownToLine size={14} />
          更新到 {tag}
        </Button>
      );
    if (project.state === "fresh")
      return (
        <Button
          size={wide ? "default" : "sm"}
          className={className}
          disabled={busy || !connected}
          onClick={() => download(project)}
        >
          <ArrowDownToLine size={14} />
          下载 {tag}
        </Button>
      );
    if (project.state === "unwatched")
      return (
        <Button
          size={wide ? "default" : "sm"}
          className={className}
          disabled={busy || !connected}
          onClick={() => watch(project)}
        >
          <BellPlus size={14} />
          关注更新
        </Button>
      );
    return null;
  };

  const menuItems = (project: SoftwareProject) => [
    {
      label: "查看所有版本",
      icon: <ListChecks size={14} />,
      onSelect: () => openRepository(project.repository),
    },
    !!project.local && {
      label: "打开所在文件夹",
      icon: <FolderOpen size={14} />,
      onSelect: () => openFolder(project),
    },
    !!project.favorite && {
      label: "立即检查",
      icon: <RefreshCw size={14} />,
      disabled: snapshot?.checking.active,
      onSelect: () => check(project),
    },
    project.unread && {
      label: "标为已读",
      icon: <CheckCheck size={14} />,
      onSelect: () => dismiss(project),
    },
    !!project.favorite?.latest && {
      label: project.skipped ? "取消忽略此版本" : "忽略此版本",
      icon: <EyeOff size={14} />,
      onSelect: () => skip(project),
    },
    !!project.favorite && {
      label: "项目设置",
      icon: <Settings2 size={14} />,
      onSelect: () => setEditing(project),
    },
    project.favorite
      ? {
          label: "取消关注",
          icon: <Trash2 size={14} />,
          danger: true,
          onSelect: () => unwatch(project),
        }
      : {
          label: "关注更新",
          icon: <BellPlus size={14} />,
          onSelect: () => watch(project),
        },
  ];

  return (
    <section className="page sw-page" tabIndex={-1}>
      <PageHeader>
        <h1>
          我的软件
          {projects.length > 0 && (
            <span className="count-label">
              <RollingNumber value={projects.length} />
            </span>
          )}
        </h1>
        <div className="header-actions">
          <div className="header-stats" aria-label="软件概况">
            <span className={updates ? "is-accent" : ""}>
              <strong>
                <RollingNumber value={updates} />
              </strong>
              个可更新
            </span>
            <span>
              <strong>
                <RollingNumber value={snapshot?.favorites.length ?? 0} />
              </strong>
              个已关注
            </span>
            {issues > 0 && (
              <span className="is-warning">
                <strong>
                  <RollingNumber value={issues} />
                </strong>
                个文件异常
              </span>
            )}
          </div>
          <WatchlistTransfer />
          {updates > 1 && (
            <Button disabled={busy || !connected} onClick={updateAll}>
              <ArrowDownToLine size={15} />
              <span className="collapsible-label">全部更新</span>
              <span className="count-label">
                <RollingNumber value={updates} />
              </span>
            </Button>
          )}
          <Button
            disabled={
              busy ||
              !connected ||
              !snapshot?.favorites.length ||
              snapshot?.checking.active
            }
            onClick={() => check()}
            aria-label="检查全部关注项目的更新"
          >
            <RefreshCw
              size={15}
              className={snapshot?.checking.active ? "spin" : ""}
            />
            <span className="collapsible-label">检查更新</span>
          </Button>
          <Button
            variant="default"
            disabled={!connected}
            onClick={() => setAdding(true)}
          >
            <Plus size={16} />
            添加软件
          </Button>
        </div>
      </PageHeader>

      {snapshot?.checking.active && (
        <div className="checking-banner">
          <RefreshCw size={16} className="spin" />
          <span>
            正在检查 {snapshot.checking.current} · {snapshot.checking.done}/
            {snapshot.checking.total}
          </span>
          <Button
            size="sm"
            variant="ghost"
            onClick={() =>
              request("favoriteCancel").catch((reason) =>
                notify(String(reason), "error"),
              )
            }
          >
            停止
          </Button>
        </div>
      )}

      <div className="sw-tabs-row">
      <div
        className="sw-tabs"
        role="tablist"
        aria-label="我的软件视图"
        ref={viewThumb.root}
      >
        <span className="tab-thumb" ref={viewThumb.thumb} aria-hidden="true" />
        <button
          role="tab"
          aria-selected={tab === "software"}
          className={tab === "software" ? "selected" : ""}
          onClick={() => setTab("software")}
        >
          软件
        </button>
        <button
          role="tab"
          aria-selected={tab === "feed"}
          className={tab === "feed" ? "selected" : ""}
          onClick={() => setTab("feed")}
        >
          更新动态
          {unreadCount > 0 && (
            <small>
              <RollingNumber value={unreadCount} />
            </small>
          )}
        </button>
      </div>
      <button className="sw-schedule" onClick={() => navigate("settings")}>
        {snapshot?.settings.autoCheck
          ? `每 ${formatInterval(snapshot.settings.watchIntervalMs)}自动检查`
          : "仅手动检查"}
      </button>
      </div>

      {tab === "feed" ? (
        <div className="panel sw-feed">
          <UpdateFeed active={visible && tab === "feed"} />
        </div>
      ) : (
        <div className={`workbench sw-workbench ${selected ? "has-inspector" : ""}`}>
          <div className="task-list-pane">
            <div className="list-toolbar">
              <div
                className="tabs"
                role="tablist"
                aria-label="筛选软件"
                ref={filterThumb.root}
              >
                <span className="tab-thumb" ref={filterThumb.thumb} aria-hidden="true" />
                {filters.map(([key, label]) => {
                  const count = projects.filter((project) =>
                    matches(project, key),
                  ).length;
                  if (key === "issue" && !count) return null;
                  return (
                    <button
                      key={key}
                      role="tab"
                      aria-selected={filter === key}
                      className={filter === key ? "selected" : ""}
                      onClick={() => {
                        setFilter(key);
                        setPage(1);
                      }}
                    >
                      {label}
                      {key !== "all" && count > 0 && (
                        <span className="tab-count">
                          <RollingNumber value={count} />
                        </span>
                      )}
                    </button>
                  );
                })}
              </div>
              <div className="sw-search">
                {groups.length > 0 && (
                  <select
                    aria-label="按分组筛选"
                    value={group}
                    onChange={(event) => {
                      setGroup(event.target.value);
                      setPage(1);
                    }}
                  >
                    <option value="">全部分组</option>
                    {groups.map((value) => (
                      <option key={value}>{value}</option>
                    ))}
                  </select>
                )}
                <label className="search-box">
                  <Search size={15} />
                  <Input
                    aria-label="搜索软件"
                    placeholder="搜索软件"
                    value={query}
                    onChange={(event) => {
                      setQuery(event.target.value);
                      setPage(1);
                    }}
                  />
                </label>
              </div>
            </div>
            {error && <ErrorBox text={`读取本地文件失败：${error}`} />}
            {shown.length > 0 && (
              <div className="sw-head" aria-hidden="true">
                <span />
                <span>软件</span>
                <div className="sw-versions">
                  <span>本地版本</span>
                  <span />
                  <span>最新版本</span>
                </div>
                <span />
              </div>
            )}
            <div className="task-list">
              {!shown.length ? (
                locals === null && connected && !error ? null : (
                  <Empty
                    icon={<Package size={24} strokeWidth={1.5} />}
                    title={
                      projects.length ? "没有符合条件的软件" : "还没有软件"
                    }
                    description={
                      projects.length
                        ? "换个筛选条件或关键词试试。"
                        : "添加一个 GitHub 项目，或者下载过的软件会自动出现在这里。"
                    }
                    action={
                      !projects.length && (
                        <Button
                          disabled={!connected}
                          onClick={() => setAdding(true)}
                        >
                          <Plus size={15} />
                          添加软件
                        </Button>
                      )
                    }
                  />
                )
              ) : (
                pagedProjects.map((project) => {
                  const [owner, ...rest] = project.repository.split("/");
                  const latest = project.favorite?.latest;
                  const labels = [
                    project.entry?.group,
                    ...(project.entry?.tags ?? []),
                  ].filter(Boolean);
                  return (
                    <article
                      key={project.key}
                      className={`sw-row ${selected?.key === project.key ? "is-selected" : ""}`}
                    >
                      <div className={`sw-icon sw-icon-${project.state}`}>
                        <Package size={18} strokeWidth={1.6} />
                      </div>
                      <div className="sw-main">
                        <div className="sw-name">
                          <button
                            className="task-name-button"
                            title={project.repository}
                            aria-label={`查看 ${project.repository} 的详情`}
                            aria-expanded={selected?.key === project.key}
                            onClick={(event) => {
                              detailTrigger.current = event.currentTarget;
                              select(project.key);
                            }}
                          >
                            {rest.join("/")}
                          </button>
                          {project.unread && (
                            <i className="unread-dot" aria-label="有未读的新版本" />
                          )}
                        </div>
                        <div className="sw-owner">
                          <span>{owner}</span>
                          {labels.map((label) => (
                            <span className="sw-label" key={label}>
                              {label}
                            </span>
                          ))}
                        </div>
                      </div>
                      <div className="sw-versions">
                        <div>
                          <small>本地</small>
                          <strong className={project.local ? "" : "muted"}>
                            {project.local?.tag ?? (project.local ? "版本未知" : "未下载")}
                          </strong>
                          {project.fileIssue && (
                            <span
                              className="sw-file-issue"
                              data-file-state={project.local!.fileState}
                            >
                              {fileStateLabels[project.local!.fileState] ?? "位置未确认"}
                            </span>
                          )}
                        </div>
                        <ArrowRight
                          size={14}
                          className={`sw-arrow ${project.state === "update" ? "is-on" : ""}`}
                          aria-hidden="true"
                        />
                        <div>
                          <small>最新</small>
                          <strong className={latest ? "" : "muted"}>
                            {latest?.tag ??
                              (project.favorite
                                ? project.favorite.error
                                  ? "检查失败"
                                  : "等待检查"
                                : "未关注")}
                          </strong>
                          {latest?.publishedAt && (
                            <span className="sw-when">
                              {relativeDay(latest.publishedAt)}
                            </span>
                          )}
                        </div>
                      </div>
                      <div className="sw-actions">
                        {primary(project) ?? (
                          <span
                            className={`status ${project.state === "current" ? "status-completed" : ""}`}
                          >
                            {project.skipped && project.state === "current"
                              ? "已忽略新版本"
                              : stateLabels[project.state]}
                          </span>
                        )}
                        <Menu
                          label={`${project.repository} 的更多操作`}
                          disabled={busy}
                          items={menuItems(project)}
                        />
                      </div>
                    </article>
                  );
                })
              )}
            </div>
            {shown.length > 0 && (
              <Pager
                page={listPage}
                total={shown.length}
                pageSize={pageSize}
                onPage={setPage}
                onPageSize={setPageSize}
                unit="个软件"
              />
            )}
          </div>
          {selected && (
            <SoftwareInspector
              project={selected}
              busy={busy}
              primary={primary(selected, true)}
              onClose={closeDetails}
              onOpenFolder={() => openFolder(selected)}
              onEdit={() => setEditing(selected)}
              onUnwatch={() => unwatch(selected)}
              onRun={run}
            />
          )}
        </div>
      )}

      <Modal
        open={adding}
        onClose={() => setAdding(false)}
        busy={busy}
        title="添加软件"
        description="粘贴 GitHub 项目或 Release 页面地址，之后发布的新版本会提醒你。"
        footer={
          <>
            <Button disabled={busy} onClick={() => setAdding(false)}>
              取消
            </Button>
            <Button
              variant="default"
              disabled={busy || !input.trim()}
              onClick={() =>
                run(async () => {
                  await request("favoriteAdd", { input: input.trim() });
                  if (active.current) {
                    setAdding(false);
                    setInput("");
                  }
                })
              }
            >
              {busy ? "正在添加…" : "添加并关注"}
            </Button>
          </>
        }
      >
        <label className="field-label" htmlFor="software-url">
          项目地址
        </label>
        <Input
          id="software-url"
          placeholder="https://github.com/owner/repository"
          value={input}
          disabled={busy}
          onChange={(event) => setInput(event.target.value)}
          onPaste={(event) => pasteSource(event, setInput)}
          onKeyDown={(event) => {
            if (event.key === "Enter" && !event.nativeEvent.isComposing && input.trim())
              run(async () => {
                await request("favoriteAdd", { input: input.trim() });
                if (active.current) {
                  setAdding(false);
                  setInput("");
                }
              });
          }}
          autoFocus
        />
      </Modal>

      {editing && (
        <ProjectSettings
          project={editing}
          onClose={() => setEditing(null)}
        />
      )}
    </section>
  );
}

function formatInterval(ms: number) {
  return ms < 3_600_000 ? `${ms / 60_000} 分钟` : `${ms / 3_600_000} 小时`;
}

function SoftwareInspector({
  project,
  busy,
  primary,
  onClose,
  onOpenFolder,
  onEdit,
  onUnwatch,
  onRun,
}: {
  project: SoftwareProject;
  busy: boolean;
  primary: ReactNode;
  onClose(): void;
  onOpenFolder(): void;
  onEdit(): void;
  onUnwatch(): void;
  onRun(fn: () => Promise<unknown>): Promise<void>;
}) {
  const { snapshot } = useWorkspace();
  const close = useRef<HTMLButtonElement>(null);
  const [events, setEvents] = useState<UpdateEvent[] | null>(null);
  const [notes, setNotes] = useState<string | null>(null);
  const { favorite, entry, local } = project;
  const latest = favorite?.latest;
  useEffect(() => {
    close.current?.focus({ preventScroll: true });
  }, [project.key]);
  useEffect(() => {
    setEvents(null);
    setNotes(null);
    if (!favorite) return;
    let cancelled = false;
    request<UpdatePage>("updates", {
      cursor: null,
      limit: 20,
      repository: favorite.repository,
      recent: true,
    })
      .then(async (page) => {
        if (cancelled) return;
        const sorted = [...page.events].sort((a, b) => b.id - a.id);
        setEvents(sorted);
        const current = sorted.find((event) => event.release.id === latest?.id);
        if (current) {
          const detail = await request<UpdateEvent>("updateDetail", {
            id: current.id,
          });
          if (!cancelled) setNotes(detail.notes);
        }
      })
      .catch(() => {
        if (!cancelled) setEvents([]);
      });
    return () => {
      cancelled = true;
    };
  }, [favorite?.repository, latest?.id]);
  const rule = entry?.rule;
  const autoDownload =
    rule?.autoDownloadOverride ??
    ((snapshot?.settings.watchAutoDownload ?? false) || !!rule?.autoDownload);
  return (
    <aside
      className="inspector sw-inspector"
      aria-label="软件详情"
      onKeyDown={(event) => {
        if (event.key === "Escape" && !event.defaultPrevented) {
          event.stopPropagation();
          onClose();
        }
      }}
    >
      <div className="inspector-title">
        软件详情
        <span
          className={`status ${project.state === "update" ? "status-downloading" : project.state === "current" ? "status-completed" : ""}`}
        >
          {stateLabels[project.state]}
        </span>
        <Button
          ref={close}
          size="icon"
          variant="ghost"
          aria-label="收起软件详情"
          onClick={onClose}
        >
          <X size={17} />
        </Button>
      </div>
      <div className="inspector-file">
        <h2>{project.repository}</h2>
        {!!(entry?.group || entry?.tags?.length) && (
          <p>{[entry?.group, ...(entry?.tags ?? [])].filter(Boolean).join(" · ")}</p>
        )}
      </div>
      <div className="detail-stats">
        <div>
          <small>本地版本</small>
          <strong>{local?.tag ?? (local ? "版本未知" : "未下载")}</strong>
        </div>
        <div>
          <small>最新版本</small>
          <strong>{latest?.tag ?? (favorite ? "等待检查" : "未关注")}</strong>
        </div>
      </div>
      {favorite?.error && <div className="detail-error">{favorite.error}</div>}
      {primary && <div className="sw-inspector-primary">{primary}</div>}

      {local && (
        <section className="sw-section">
          <h3>本地文件</h3>
          <dl className="details">
            <dt>文件</dt>
            <dd>
              {local.filename} · {bytes(local.bytes)}
            </dd>
            <dt>位置</dt>
            <dd className="break-all">{local.path ?? "位置未记录"}</dd>
            <dt>状态</dt>
            <dd className={project.fileIssue ? "sw-file-issue" : ""}>
              {fileStateLabels[local.fileState] ?? "位置未确认"}
            </dd>
            <dt>校验</dt>
            <dd>{verificationLabels[local.verification] ?? local.verification}</dd>
            <dt>下载于</dt>
            <dd>
              {date(local.completedAt)}
              {local.fileCount > 1 && ` · 共下载过 ${local.fileCount} 个文件`}
            </dd>
          </dl>
          <div className="inspector-actions">
            <Button disabled={busy} onClick={onOpenFolder}>
              <FolderOpen size={14} />
              打开文件夹
            </Button>
            <CopyButton
              disabled={busy || !local.path}
              text={local.path}
              label="复制路径"
            />
          </div>
        </section>
      )}

      {favorite && latest && (
        <section className="sw-section">
          <h3>
            {latest.tag} 更新说明
            <button
              className="text-link"
              onClick={() => onRun(() => request("openLink", { url: latest.url }))}
            >
              发布页
              <ExternalLink size={11} />
            </button>
          </h3>
          <div className="sw-notes">
            <Markdown>
              {notes ?? entry?.notesExcerpt ?? "这个版本没有提供更新说明。"}
            </Markdown>
          </div>
        </section>
      )}

      {favorite && !!events?.length && (
        <section className="sw-section">
          <h3>版本记录</h3>
          <ul className="sw-history">
            {events.map((event) => (
              <li key={event.id}>
                <strong>{event.release.tag}</strong>
                <span>{date(event.release.publishedAt ?? event.discoveredAt).split(" ")[0]}</span>
                <small>
                  {event.downloadedTasks.length
                    ? "已下载"
                    : event.skipped
                      ? "已忽略"
                      : ""}
                </small>
              </li>
            ))}
          </ul>
        </section>
      )}

      {favorite && (
        <section className="sw-section">
          <h3>
            关注设置
            <button className="text-link" onClick={onEdit}>
              编辑
            </button>
          </h3>
          <dl className="details">
            <dt>预发布</dt>
            <dd>{rule?.includePrerelease ? "包含" : "不包含"}</dd>
            <dt>自动下载</dt>
            <dd>{autoDownload ? "开启" : "关闭"}</dd>
            <dt>附件匹配</dt>
            <dd>{rule?.assetPattern || "使用全局规则"}</dd>
          </dl>
        </section>
      )}

      <div className="sw-inspector-footer">
        <Button
          variant="ghost"
          onClick={() => openRepository(project.repository)}
        >
          <ListChecks size={14} />
          查看所有版本
        </Button>
        {favorite && (
          <Button
            variant="ghost"
            className="danger-text"
            disabled={busy}
            onClick={onUnwatch}
          >
            <Trash2 size={14} />
            取消关注
          </Button>
        )}
      </div>
    </aside>
  );
}

/** 关注规则与分组标签合并后的「项目设置」。 */
function ProjectSettings({
  project,
  onClose,
}: {
  project: SoftwareProject;
  onClose(): void;
}) {
  const { snapshot } = useWorkspace();
  const { busy, run, active } = useJob();
  const saved = project.entry?.rule ?? {
    includePrerelease: false,
    autoDownload: false,
    assetPattern: null,
  };
  const [rule, setRule] = useState<WatchRule>({
    ...saved,
    autoDownload:
      saved.autoDownloadOverride ??
      ((snapshot?.settings.watchAutoDownload ?? false) || saved.autoDownload),
  });
  const [group, setGroup] = useState(project.entry?.group ?? "");
  const [tags, setTags] = useState(project.entry?.tags?.join(", ") ?? "");
  return (
    <Modal
      open
      onClose={onClose}
      busy={busy}
      title="项目设置"
      description={project.repository}
      footer={
        <>
          <Button disabled={busy} onClick={onClose}>
            取消
          </Button>
          <Button
            variant="default"
            disabled={busy}
            onClick={() =>
              run(async () => {
                await request("watchRule", {
                  repository: project.repository,
                  rule: {
                    ...rule,
                    downloadDirectory: rule.downloadDirectory?.trim() || null,
                  },
                });
                await request("projectLabels", {
                  repository: project.repository,
                  group,
                  tags: tags
                    .split(/[,，]/)
                    .map((value) => value.trim())
                    .filter(Boolean),
                });
                if (active.current) onClose();
              })
            }
          >
            保存
          </Button>
        </>
      }
    >
      <h3 className="sw-form-title">新版本</h3>
      <Toggle
        title="包含预发布版本"
        checked={rule.includePrerelease}
        onChange={(value) => setRule({ ...rule, includePrerelease: value })}
      />
      <Toggle
        title="自动下载新版本"
        description="开启自动检查后生效，下载会进入下载队列。"
        checked={rule.autoDownload}
        onChange={(value) => setRule({ ...rule, autoDownload: value })}
      />
      <h3 className="sw-form-title">选择附件</h3>
      <AssetPreferenceFields
        value={rule.preference ?? defaultPreference}
        disabled={busy}
        onChange={(preference) => setRule({ ...rule, preference })}
      />
      <label className="field-label" htmlFor="watch-pattern">
        附件名称匹配 <span className="muted">可选</span>
      </label>
      <Input
        id="watch-pattern"
        placeholder="例如 *windows*x64*.zip，留空使用全局规则"
        value={rule.assetPattern || ""}
        onChange={(event) =>
          setRule({ ...rule, assetPattern: event.target.value || null })
        }
      />
      <p className="form-note">
        支持 * 和 ? 通配符，以及 {"{version}"} 占位符。匹配到多个附件时需要手动选择。
      </p>
      <label className="field-label" htmlFor="watch-download-directory">
        保存位置 <span className="muted">留空使用最近一次的目录</span>
      </label>
      <Input
        id="watch-download-directory"
        value={rule.downloadDirectory ?? ""}
        disabled={busy}
        onChange={(event) =>
          setRule({ ...rule, downloadDirectory: event.target.value || null })
        }
      />
      <h3 className="sw-form-title">整理</h3>
      <label className="field-label" htmlFor="project-group">
        分组
      </label>
      <Input
        id="project-group"
        value={group}
        maxLength={40}
        onChange={(event) => setGroup(event.target.value)}
        placeholder="例如 开发工具"
      />
      <label className="field-label" htmlFor="project-tags">
        标签 <span className="muted">用逗号分隔，最多 12 个</span>
      </label>
      <Input
        id="project-tags"
        value={tags}
        onChange={(event) => setTags(event.target.value)}
        placeholder="例如 Rust, 命令行"
      />
    </Modal>
  );
}
