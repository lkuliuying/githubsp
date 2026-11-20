// 宽屏时页面右侧常驻栏里的内容：下载队列的概览、我的软件的概览、新建下载的快捷入口。
import { useEffect, useMemo, useState } from "react";
import {
  ArrowDownToLine,
  ArrowRight,
  File,
  FolderOpen,
  GitBranch,
  History,
  Package,
  RefreshCw,
} from "lucide-react";
import { navigate, notify, request, useWorkspace } from "@/lib/api";
import {
  bytes,
  relativeDay,
  relativeTime,
  type HistoryPage,
  type LocalSoftware,
  type Task,
  type UpdatePage,
  type UpdateEvent,
} from "@/lib/model";
import { buildProjects, type SoftwareProject } from "@/lib/software";
import { RailCard, Sparkline, useSpeedSamples } from "@/components/PageRail";
import { Button } from "@/components/ui/button";
import { useConfirm, useJob } from "@/components/common";
import { openRepository } from "./NewDownload";

type Confirm = ReturnType<typeof useConfirm>;

/** 按关注规则自动选附件加入队列；选不出来时询问是否打开版本列表手动选。 */
export async function downloadFavorite(repository: string, confirm: Confirm) {
  try {
    await request("favoriteDownload", { repository });
    notify(`${repository} 已加入下载队列`, "success");
  } catch (reason) {
    const message = reason instanceof Error ? reason.message : String(reason);
    if (
      await confirm(
        "没能自动选出附件",
        `${message}\n\n可以打开版本列表手动选择要下载的附件。`,
        "手动选择",
      )
    )
      openRepository(repository);
  }
}

/** 最近下载完成的记录（按添加时间从新到旧的第一页）。 */
function useCompletedHistory(visible: boolean, pageSize: number) {
  const { snapshot, connected } = useWorkspace();
  const [page, setPage] = useState<HistoryPage | null>(null);
  useEffect(() => {
    if (!visible || !connected) return;
    let cancelled = false;
    request<HistoryPage>("history", {
      query: "",
      status: "completed",
      page: 1,
      pageSize,
    })
      .then((value) => {
        if (!cancelled) setPage(value);
      })
      .catch(() => undefined);
    return () => {
      cancelled = true;
    };
  }, [visible, connected, snapshot?.historyRevision, pageSize]);
  return page;
}

/** 本地下载过的软件，用来判断哪些关注项目可以更新。 */
function useLocalSoftware(visible: boolean) {
  const { snapshot, connected } = useWorkspace();
  const [locals, setLocals] = useState<LocalSoftware[] | null>(null);
  useEffect(() => {
    if (!visible || !connected) return;
    let cancelled = false;
    request<LocalSoftware[]>("softwareProjects")
      .then((value) => {
        if (!cancelled) setLocals(value);
      })
      .catch(() => undefined);
    return () => {
      cancelled = true;
    };
  }, [visible, connected, snapshot?.historyRevision]);
  return locals;
}

const startOfToday = () => new Date().setHours(0, 0, 0, 0);
const lastChecked = (projects: SoftwareProject[]) =>
  Math.max(0, ...projects.map((project) => project.favorite?.lastChecked ?? 0));

function RepoName({ repository }: { repository: string }) {
  const [owner, ...rest] = repository.split("/");
  return (
    <span className="rail-repo" title={repository}>
      <strong>{rest.join("/") || owner}</strong>
      {rest.length > 0 && <small>{owner}</small>}
    </span>
  );
}

/** 可更新的软件列表，下载队列和我的软件共用。 */
function UpdatesCard({
  projects,
  busy,
  onDownload,
  onUpdateAll,
  onSelect,
  limit,
}: {
  projects: SoftwareProject[];
  busy: boolean;
  onDownload(project: SoftwareProject): void;
  onUpdateAll?(): void;
  onSelect?(project: SoftwareProject): void;
  limit: number;
}) {
  const { snapshot, connected } = useWorkspace();
  const updates = projects.filter((project) => project.state === "update");
  const watched = projects.filter((project) => project.favorite).length;
  const checked = lastChecked(projects);
  return (
    <RailCard
      title={updates.length ? "我的软件有更新" : "我的软件"}
      extra={
        updates.length > 0 && (
          <span className="rail-badge">{updates.length} 个可更新</span>
        )
      }
    >
      {updates.length ? (
        <>
          <ul className="rail-list">
            {updates.slice(0, limit).map((project) => (
              <li key={project.key}>
                <div className="rail-item-main">
                  {onSelect ? (
                    <button
                      className="rail-link-button"
                      onClick={() => onSelect(project)}
                      aria-label={`查看 ${project.repository} 的详情`}
                    >
                      <RepoName repository={project.repository} />
                    </button>
                  ) : (
                    <RepoName repository={project.repository} />
                  )}
                  <span className="rail-meta">
                    {project.local?.tag ?? "版本未知"}
                    <ArrowRight size={11} aria-hidden="true" />
                    <span className="rail-meta-strong">
                      {project.favorite?.latest?.tag}
                    </span>
                  </span>
                </div>
                <Button
                  size="sm"
                  disabled={busy || !connected}
                  aria-label={`下载 ${project.repository} 的新版本`}
                  onClick={() => onDownload(project)}
                >
                  下载
                </Button>
              </li>
            ))}
          </ul>
          <footer className="rail-card-foot">
            {updates.length > limit ? (
              <button className="pager-link" onClick={() => navigate("software")}>
                还有 {updates.length - limit} 个 →
              </button>
            ) : (
              <span />
            )}
            {onUpdateAll && updates.length > 1 && (
              <Button
                size="sm"
                variant="default"
                disabled={busy || !connected}
                onClick={onUpdateAll}
              >
                <ArrowDownToLine size={13} />
                全部更新
              </Button>
            )}
            {!onUpdateAll && updates.length <= limit && (
              <button className="pager-link" onClick={() => navigate("software")}>
                查看全部 →
              </button>
            )}
          </footer>
        </>
      ) : watched ? (
        <div className="rail-note">
          <p>关注的 {watched} 个软件都是最新版本。</p>
          <small>
            {snapshot?.checking.active
              ? "正在检查更新…"
              : checked
                ? `上次检查：${relativeTime(checked)}`
                : "还没有检查过"}
          </small>
          <Button
            size="sm"
            variant="ghost"
            disabled={busy || !connected || snapshot?.checking.active}
            onClick={() =>
              request("favoriteCheck", { repository: null }).catch((reason) =>
                notify(String(reason), "error"),
              )
            }
          >
            <RefreshCw
              size={13}
              className={snapshot?.checking.active ? "spin" : ""}
            />
            检查更新
          </Button>
        </div>
      ) : (
        <div className="rail-note">
          <p>关注常用软件后，有新版本会在这里提醒你。</p>
          <Button size="sm" onClick={() => navigate("software")}>
            <Package size={13} />
            去我的软件
          </Button>
        </div>
      )}
    </RailCard>
  );
}

/** 下载队列右侧：今日概览、可更新的软件、最近完成。 */
export function QueueOverview({
  visible,
  tasks,
  onShowDone,
}: {
  visible: boolean;
  tasks: Task[];
  onShowDone(): void;
}) {
  const { snapshot } = useWorkspace();
  const { busy, run } = useJob();
  const confirm = useConfirm();
  const speed = tasks.reduce((sum, task) => sum + task.speed, 0);
  const samples = useSpeedSamples(speed, visible);
  const history = useCompletedHistory(visible, 100);
  const locals = useLocalSoftware(visible);
  const projects = useMemo(
    () => (snapshot && locals ? buildProjects(snapshot, locals) : []),
    [snapshot, locals],
  );
  const items = history?.items ?? [];
  const today = startOfToday();
  const finishedToday = items.filter(
    ({ task }) => (task.completedAt ?? 0) >= today,
  );
  // 第一页全是今天完成的，说明今天可能还有更多，数字后面加「+」。
  const more =
    !!history && finishedToday.length === items.length && history.total > items.length;
  const todayBytes = finishedToday.reduce(
    (sum, { task }) => sum + (task.total ?? task.downloaded),
    0,
  );
  const recent = [...items]
    .sort((a, b) => (b.task.completedAt ?? 0) - (a.task.completedAt ?? 0))
    .slice(0, 4);
  return (
    <>
      <RailCard title="今日概览">
        <div className="rail-stats">
          <div>
            <small>今日已下载</small>
            <strong>
              {bytes(todayBytes)}
              {more && "+"}
            </strong>
          </div>
          <div>
            <small>完成</small>
            <strong>
              {finishedToday.length}
              {more && "+"} 个
            </strong>
          </div>
        </div>
        <div className="rail-speed">
          <div className="rail-speed-head">
            <small>最近 60 秒速度</small>
            <span className={speed > 0 ? "is-live" : ""}>{bytes(speed)}/s</span>
          </div>
          <Sparkline values={samples} />
        </div>
      </RailCard>
      {snapshot && locals && (
        <UpdatesCard
          projects={projects}
          busy={busy}
          limit={3}
          onDownload={(project) =>
            run(() => downloadFavorite(project.repository, confirm))
          }
        />
      )}
      <RailCard
        title="最近完成"
        extra={
          !!snapshot?.completedTasks && (
            <button className="pager-link" onClick={onShowDone}>
              全部 {snapshot.completedTasks} 个 →
            </button>
          )
        }
      >
        {recent.length ? (
          <ul className="rail-list">
            {recent.map(({ task, fileState }) => (
              <li key={task.id}>
                <span className="rail-icon">
                  <File size={15} />
                </span>
                <div className="rail-item-main">
                  <strong className="rail-name" title={task.filename}>
                    {task.filename}
                  </strong>
                  <span className="rail-meta">
                    {bytes(task.total ?? task.downloaded)} ·{" "}
                    {relativeTime(task.completedAt ?? task.createdAt)}
                  </span>
                </div>
                {fileState === "present" ? (
                  <Button
                    size="icon"
                    variant="ghost"
                    aria-label={`打开 ${task.filename} 所在文件夹`}
                    disabled={busy}
                    onClick={() =>
                      run(() => request("openDirectory", { taskId: task.id }))
                    }
                  >
                    <FolderOpen size={15} />
                  </Button>
                ) : (
                  <span className="rail-meta">文件已移动</span>
                )}
              </li>
            ))}
          </ul>
        ) : (
          <p className="rail-note">
            {history ? "下载完成的文件会出现在这里。" : "正在读取…"}
          </p>
        )}
      </RailCard>
    </>
  );
}

/** 我的软件右侧（没有选中软件时）：可更新列表和最近发布的版本。 */
export function SoftwareOverview({
  projects,
  busy,
  visible,
  showRecent,
  onDownload,
  onUpdateAll,
  onSelect,
}: {
  projects: SoftwareProject[];
  busy: boolean;
  visible: boolean;
  showRecent: boolean;
  onDownload(project: SoftwareProject): void;
  onUpdateAll(): void;
  onSelect(project: SoftwareProject): void;
}) {
  const { snapshot, connected } = useWorkspace();
  const [events, setEvents] = useState<UpdateEvent[] | null>(null);
  // 只在关注列表或最新版本变化时重新读取，下载进度刷新不触发。
  const releasesKey = (snapshot?.favorites ?? [])
    .map((favorite) => `${favorite.id}:${favorite.latest?.id ?? ""}`)
    .join(",");
  useEffect(() => {
    if (!visible || !connected || !showRecent) return;
    let cancelled = false;
    request<UpdatePage>("updates", {
      cursor: null,
      limit: 8,
      repository: null,
      recent: true,
    })
      .then((page) => {
        if (!cancelled)
          setEvents(
            [...page.events]
              .sort(
                (a, b) =>
                  (b.release.publishedAt ?? b.discoveredAt) -
                  (a.release.publishedAt ?? a.discoveredAt),
              )
              .slice(0, 6),
          );
      })
      .catch(() => undefined);
    return () => {
      cancelled = true;
    };
  }, [visible, connected, showRecent, releasesKey]);
  const byKey = new Map(projects.map((project) => [project.key, project]));
  return (
    <>
      {/* 软件列表本身已经把可更新的排在最前，只有切到「更新动态」时才在这里列出。 */}
      {!showRecent && (
        <UpdatesCard
          projects={projects}
          busy={busy}
          limit={5}
          onDownload={onDownload}
          onUpdateAll={onUpdateAll}
          onSelect={onSelect}
        />
      )}
      {showRecent && (
        <RailCard title="最近发布">
          {events?.length ? (
            <ul className="rail-list">
              {events.map((event) => {
                const project = byKey.get(event.repository.toLowerCase());
                return (
                  <li key={event.id}>
                    <div className="rail-item-main">
                      {project ? (
                        <button
                          className="rail-link-button"
                          onClick={() => onSelect(project)}
                          aria-label={`查看 ${event.repository} 的详情`}
                        >
                          <RepoName repository={event.repository} />
                        </button>
                      ) : (
                        <RepoName repository={event.repository} />
                      )}
                      <span className="rail-meta">
                        <span className="rail-meta-strong">{event.release.tag}</span>
                        {event.prerelease && " · 预发布"}
                      </span>
                    </div>
                    <span className="rail-meta">
                      {relativeDay(event.release.publishedAt ?? event.discoveredAt)}
                    </span>
                  </li>
                );
              })}
            </ul>
          ) : (
            <p className="rail-note">
              {events ? "关注的软件发布新版本后会出现在这里。" : "正在读取…"}
            </p>
          )}
        </RailCard>
      )}
      {showRecent && projects.length > 0 && (
        <p className="rail-tip">点软件名可以在这里查看详情和更新说明。</p>
      )}
    </>
  );
}

/** 新建下载右侧：从关注的软件、最近下载过的仓库直接读取版本，以及能粘贴哪些地址。 */
export function NewDownloadRail({ visible }: { visible: boolean }) {
  const { snapshot } = useWorkspace();
  const history = useCompletedHistory(visible, 60);
  const favorites = snapshot?.favorites ?? [];
  const watched = new Set(favorites.map((item) => item.repository.toLowerCase()));
  const recent: { repository: string; when: number }[] = [];
  for (const { task } of history?.items ?? []) {
    if (!task.repository || watched.has(task.repository.toLowerCase())) continue;
    if (recent.some((item) => item.repository.toLowerCase() === task.repository!.toLowerCase()))
      continue;
    recent.push({ repository: task.repository, when: task.completedAt ?? task.createdAt });
    if (recent.length >= 4) break;
  }
  return (
    <>
      <RailCard
        title="从我的软件挑选"
        extra={
          favorites.length > 5 && (
            <button className="pager-link" onClick={() => navigate("software")}>
              全部 {favorites.length} 个 →
            </button>
          )
        }
      >
        {favorites.length ? (
          <ul className="rail-list">
            {favorites.slice(0, 5).map((favorite) => (
              <li key={favorite.id}>
                <button
                  className="rail-row-button"
                  onClick={() => openRepository(favorite.repository)}
                  aria-label={`读取 ${favorite.repository} 的版本`}
                >
                  <span className="rail-icon">
                    <Package size={15} />
                  </span>
                  <div className="rail-item-main">
                    <RepoName repository={favorite.repository} />
                    <span className="rail-meta">
                      {favorite.latest ? `最新 ${favorite.latest.tag}` : "等待检查"}
                    </span>
                  </div>
                  <ArrowRight size={14} className="rail-row-arrow" aria-hidden="true" />
                </button>
              </li>
            ))}
          </ul>
        ) : (
          <p className="rail-note">关注的软件会列在这里，点一下就能读取版本。</p>
        )}
      </RailCard>
      {recent.length > 0 && (
        <RailCard title="最近下载过">
          <ul className="rail-list">
            {recent.map((item) => (
              <li key={item.repository}>
                <button
                  className="rail-row-button"
                  onClick={() => openRepository(item.repository)}
                  aria-label={`读取 ${item.repository} 的版本`}
                >
                  <span className="rail-icon">
                    <History size={15} />
                  </span>
                  <div className="rail-item-main">
                    <RepoName repository={item.repository} />
                    <span className="rail-meta">{relativeDay(item.when)}下载</span>
                  </div>
                  <ArrowRight size={14} className="rail-row-arrow" aria-hidden="true" />
                </button>
              </li>
            ))}
          </ul>
        </RailCard>
      )}
      <RailCard title="可以粘贴这些地址">
        <ul className="rail-help">
          <li>
            <GitBranch size={14} aria-hidden="true" />
            <div>
              <strong>仓库地址</strong>
              <code>github.com/作者/项目</code>
            </div>
          </li>
          <li>
            <GitBranch size={14} aria-hidden="true" />
            <div>
              <strong>Release 页面</strong>
              <code>…/releases/tag/v1.0</code>
            </div>
          </li>
          <li>
            <ArrowDownToLine size={14} aria-hidden="true" />
            <div>
              <strong>附件链接</strong>
              <span>每行一个，一次可以粘贴多个</span>
            </div>
          </li>
          <li>
            <File size={14} aria-hidden="true" />
            <div>
              <strong>视频简介、帖子里的地址</strong>
              <span>整段粘贴进来，会自动找出 GitHub 地址</span>
            </div>
          </li>
        </ul>
      </RailCard>
    </>
  );
}
