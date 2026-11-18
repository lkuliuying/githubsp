import { unread, type Favorite, type LocalSoftware, type Snapshot, type WatchEntry } from "./model";

/**
 * 项目在「我的软件」里的状态：
 * - update：已关注，本地版本不是最新且最新版本没被忽略
 * - fresh：已关注，有最新版本但从没下载过
 * - current：本地已是最新（或最新版本被忽略）
 * - pending：已关注，还没检查到版本
 * - unwatched：只下载过，没有关注
 */
export type SoftwareState = "update" | "fresh" | "current" | "pending" | "unwatched";

export interface SoftwareProject {
  key: string;
  repository: string;
  favorite?: Favorite;
  entry?: WatchEntry;
  local?: LocalSoftware;
  state: SoftwareState;
  unread: boolean;
  skipped: boolean;
  fileIssue: boolean;
  activity: number;
}

export const stateLabels: Record<SoftwareState, string> = {
  update: "可更新",
  fresh: "未下载",
  current: "已是最新",
  pending: "等待检查",
  unwatched: "未关注",
};

const order: Record<SoftwareState, number> = {
  update: 0,
  fresh: 1,
  pending: 2,
  current: 3,
  unwatched: 4,
};

export function buildProjects(
  snapshot: Snapshot,
  locals: LocalSoftware[],
): SoftwareProject[] {
  const projects = new Map<string, SoftwareProject>();
  const base = (key: string, repository: string): SoftwareProject => ({
    key,
    repository,
    state: "unwatched",
    unread: false,
    skipped: false,
    fileIssue: false,
    activity: 0,
  });
  for (const favorite of snapshot.favorites) {
    const key = favorite.repository.toLowerCase();
    projects.set(key, {
      ...base(key, favorite.repository),
      favorite,
      entry: snapshot.watchState.repositories[key],
    });
  }
  for (const local of locals) {
    const key = local.repository.toLowerCase();
    const project = projects.get(key) ?? base(key, local.repository);
    projects.set(key, { ...project, local });
  }
  for (const project of projects.values()) {
    const { favorite, entry, local } = project;
    const latest = favorite?.latest;
    project.skipped = !!latest && !!entry?.skippedReleaseIds?.includes(latest.id);
    project.unread = !!favorite && unread(snapshot, favorite);
    project.fileIssue = !!local && local.fileState !== "present";
    project.state = !favorite
      ? "unwatched"
      : !latest
        ? "pending"
        : !local
          ? project.skipped ? "current" : "fresh"
          : local.tag === latest.tag || project.skipped
            ? "current"
            : "update";
    project.activity = Math.max(
      local?.completedAt ?? 0,
      latest?.publishedAt ?? 0,
      favorite?.lastChecked ?? 0,
    );
  }
  return [...projects.values()].sort(
    (a, b) =>
      order[a.state] - order[b.state] ||
      Number(b.unread) - Number(a.unread) ||
      b.activity - a.activity ||
      a.repository.localeCompare(b.repository),
  );
}
