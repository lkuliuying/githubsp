export type TaskStatus =
  | "queued"
  | "probing"
  | "downloading"
  | "retrying"
  | "waiting_network"
  | "verifying"
  | "pausing"
  | "cancelling"
  | "paused"
  | "completed"
  | "failed"
  | "cancelled";
export interface Task {
  id: string;
  filename: string;
  url: string;
  directory: string;
  status: TaskStatus;
  downloaded: number;
  total: number | null;
  speed: number;
  eta: number | null;
  route: string | null;
  verification: string;
  error: string | null;
  finalPath: string | null;
  createdAt: number;
  revision: number;
  repository?: string;
  tag?: string;
  preferredRoute?: string | null;
  completedAt?: number;
  elapsedMs?: number | null;
  elapsedIsPartial?: boolean;
  queuePosition?: number;
  failure?: { category: string; message: string; action: string };
  retryInfo?: {
    reason: string;
    attempt: number;
    maxAttempts: number;
    retryInMs: number;
  };
  recoveryInfo?: {
    remainingMs: number;
    retryInMs: number;
    waitingForSlot: boolean;
  };
  routeSuggestion?: {
    id: string;
    routeId: string;
    routeName: string;
    currentSeconds: number;
    suggestedSeconds: number;
    restartBytes: number;
  };
  routeFailures?: { routeId: string; routeName: string; message: string }[];
}
export interface Settings {
  limitKib: number;
  closeToTray: boolean;
  autoCheck: boolean;
  backgroundCompletionNotice: boolean;
  watchIntervalMs: number;
  watchNotifications: boolean;
  watchAutoDownload: boolean;
  watchIncludePrerelease: boolean;
  watchAssetMode: number;
  watchAssetPattern: string;
  logEnabled: boolean;
  reduceMotion: boolean;
  autoMatchPlatform: boolean;
}

// 命令返回和状态推送的 JSON 键顺序不同，设置是否相同只取决于字段值。
export function settingsFingerprint(settings: Settings | null | undefined) {
  return JSON.stringify(
    Object.entries(settings ?? {}).sort(([a], [b]) => a.localeCompare(b)),
  );
}
export interface WatchRule {
  includePrerelease: boolean;
  autoDownload: boolean;
  assetPattern: string | null;
}
export interface WatchEntry {
  ackedReleaseId: number | null;
  notesExcerpt: string | null;
  publishedAt: number | null;
  rule: WatchRule;
}
export interface Favorite {
  id: string;
  repository: string;
  latest: {
    id: number;
    tag: string;
    url: string;
    publishedAt: number | null;
  } | null;
  lastChecked: number | null;
  nextCheck: number | null;
  error: string | null;
}
export interface Snapshot {
  revision: number;
  queueRevision: number;
  historyRevision: number;
  tasks: Task[];
  totalTasks: number;
  completedTasks: number;
  lastDirectory: string | null;
  settings: Settings;
  favorites: Favorite[];
  error: string | null;
  watchState: { repositories: Record<string, WatchEntry> };
  checking: {
    active: boolean;
    total: number;
    done: number;
    current: string | null;
  };
  notices: {
    id: string;
    kind: string;
    taskId: string | null;
    message: string;
    createdAt: number;
  }[];
  diagnostics: {
    id: string;
    name: string;
    available: boolean;
    bytesPerSecond: number;
    error: string | null;
    checkedAt: number;
  }[];
  diagnosing: boolean;
  apiStatus: {
    remaining: number | null;
    limit: number | null;
    resetAt: number | null;
    source: string | null;
    error: string | null;
  };
}
export interface Bootstrap {
  snapshot: Snapshot;
  directory: string;
  version: string;
  migrationAllowed: boolean;
  systemReduceMotion: boolean;
  locationState: { message: string | null; pending: unknown } | null;
}
export interface Asset {
  id: number;
  name: string;
  size: number;
  url: string;
  hints: string[];
  unavailable: string | null;
}
export interface Release {
  id: number;
  tag: string;
  name: string;
  prerelease: boolean;
  url: string;
  notes: string;
  assets: Asset[];
  publishedAt: number | null;
}
export interface CatalogPage {
  repository: string;
  releases: Release[];
  page: number;
  hasMore: boolean;
}
export interface BatchItem {
  input: string;
  url: string | null;
  filename: string | null;
  size: number | null;
  status: string;
  message: string | null;
  taskId: string | null;
}
export interface BatchPreview {
  items: BatchItem[];
  knownSize: number;
  unknownCount: number;
  preflight: {
    directory: string;
    available: number | null;
    required: number | null;
  };
}
export interface HistoryPage {
  items: { task: Task; fileState: string }[];
  total: number;
  page: number;
  pageSize: number;
  revision: number;
}
export interface UpdateResult {
  status: string;
  current: string;
  latest: string | null;
  message: string;
  notes: string | null;
  latestRelease: Release | null;
}

export const statusLabels: Record<TaskStatus, string> = {
  queued: "等待下载",
  probing: "检测线路",
  downloading: "下载中",
  retrying: "重试中",
  waiting_network: "等待网络",
  verifying: "校验中",
  pausing: "正在暂停",
  cancelling: "正在取消",
  paused: "已暂停",
  completed: "已完成",
  failed: "下载失败",
  cancelled: "已取消",
};
export const running = (status: TaskStatus) =>
  [
    "probing",
    "downloading",
    "retrying",
    "verifying",
    "pausing",
    "cancelling",
    "waiting_network",
  ].includes(status);
export const terminal = (status: TaskStatus) =>
  ["completed", "cancelled"].includes(status);
export const unread = (snapshot: Snapshot, favorite: Favorite) =>
  !!favorite.latest &&
  snapshot.watchState.repositories[favorite.repository.toLowerCase()]
    ?.ackedReleaseId !== favorite.latest.id;
export function bytes(value: number | null | undefined) {
  if (value == null) return "大小未知";
  if (value < 1024) return `${Math.round(value)} B`;
  const unit = Math.min(Math.floor(Math.log(value) / Math.log(1024)), 4);
  return `${(value / 1024 ** unit).toFixed(unit > 1 ? 1 : 0)} ${["B", "KB", "MB", "GB", "TB"][unit]}`;
}
export function duration(value: number | null | undefined) {
  if (value == null) return "—";
  const seconds = Math.max(0, Math.round(value));
  return seconds < 60
    ? `${seconds} 秒`
    : seconds < 3600
      ? `${Math.floor(seconds / 60)} 分 ${seconds % 60} 秒`
      : `${Math.floor(seconds / 3600)} 小时 ${Math.floor((seconds % 3600) / 60)} 分`;
}
export const date = (value: number | null | undefined) =>
  value
    ? new Date(value).toLocaleString("zh-CN", {
        year: "numeric",
        month: "2-digit",
        day: "2-digit",
        hour: "2-digit",
        minute: "2-digit",
        hour12: false,
      })
    : "尚无记录";
export function progress(task: Task) {
  return task.total && task.total > 0
    ? Math.max(0, Math.min(100, (task.downloaded / task.total) * 100))
    : null;
}
export function parseLimit(input: string, original: number) {
  const value = input.trim() === "" ? NaN : Number(input);
  if (
    !Number.isFinite(value) ||
    (value !== 0 && (value < 0.001024 || value > 10240))
  )
    throw new Error("请输入 0（不限速）或 0.001024 至 10240 MB/s 之间的数值");
  if (Number(((original * 1024) / 1_000_000).toFixed(6)) === value)
    return original;
  return Math.round((value * 1_000_000) / 1024);
}
