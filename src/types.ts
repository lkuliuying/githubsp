export type TaskStatus = 'queued' | 'probing' | 'downloading' | 'retrying' | 'waiting_network' | 'verifying' | 'pausing' | 'cancelling' | 'paused' | 'completed' | 'failed' | 'cancelled'
export type TaskAction = 'pause' | 'resume' | 'cancel' | 'remove'
export interface DirectoryInspection { directory: string; state: 'existing' | 'missing' }
export type PrepareDirectory = (directory: string, isCurrent: () => boolean) => Promise<string | null>

export interface DownloadTask {
  id: string
  url: string
  filename: string
  directory: string
  status: TaskStatus
  downloaded: number
  total: number | null
  speed: number
  eta: number | null
  route: string | null
  verification: 'pending' | 'verified' | 'unverified' | 'failed'
  error: string | null
  finalPath: string | null
  createdAt: number
  revision: number
  repository?: string | null
  tag?: string | null
  assetId?: number | null
  preferredRoute?: string | null
  failure?: { category: string; message: string; action: string } | null
  completedAt?: number | null
  elapsedMs?: number | null
  elapsedIsPartial?: boolean
  queuePosition?: number
  retryInfo?: { phase: 'retrying' | 'recovering' | 'switching'; attempt: number; maxAttempts: number; reason: string; retryInMs: number } | null
  recoveryInfo?: { remainingMs: number; retryInMs: number; waitingForSlot: boolean } | null
  routeSuggestion?: { id: string; routeId: string; routeName: string; currentSeconds: number; suggestedSeconds: number; restartBytes: number } | null
  routeFailures?: { routeId: string; routeName: string; message: string; temporary: boolean }[]
}

export interface Snapshot {
  tasks: DownloadTask[]
  lastDirectory: string | null
  error: string | null
  revision: number
  settings: Settings
  queueRevision: number
  diagnostics: RouteReport[]
  diagnosticContext?: DiagnosticContext | null
  diagnosing: boolean
  notices: Notice[]
  favorites: Favorite[]
}

export interface Settings { limitKib: number; closeToTray: boolean; autoCheck: boolean; backgroundCompletionNotice: boolean }
export interface Notice { id: string; kind: 'download_completed' | 'download_failed' | 'favorite_updated'; taskId: string | null; message: string; createdAt: number }
export interface DownloadNoticeView { revision: number; notice: { count: number; filename: string; elapsedMs: number | null; elapsedIsPartial: boolean } | null }
export interface Favorite { id: string; repository: string; latest: { id: number; tag: string; url: string } | null; lastChecked: number | null; lastSuccess: number | null; nextCheck: number | null; error: string | null }
export interface UpdateResult { status: string; current: string; latest: string | null; notes: string | null; url: string | null; message: string; nextCheck: number | null }
export interface HistoryPage { items: { task: DownloadTask; fileState: 'present' | 'missing' | 'inaccessible' | 'unfinished' }[]; total: number; page: number; pageSize: number; revision: number }
export interface RouteReport { id: string; name: string; checkedAt: number; bytesPerSecond: number; available: boolean; error: string | null }
export interface DiagnosticContext { source: 'default' | 'input' | 'download'; filename: string }
export interface Asset { id: number; name: string; size: number; url: string; sha256: string | null; hints: string[]; unavailable?: string | null }
export interface Release { id: number; tag: string; name: string; prerelease: boolean; url: string; notes: string; assets: Asset[] }
export interface CatalogPage { repository: string; releases: Release[]; page: number; hasMore: boolean; selectedUrl: string | null }
export interface BatchItem { input: string; url: string | null; filename: string | null; size: number | null; status: string; message: string | null; taskId: string | null }
export interface BatchPreview { items: BatchItem[]; knownSize: number; unknownCount: number; preflight: { directory: string; available: number | null; required: number | null; warning: string | null } }
export interface BatchResult { items: BatchItem[]; snapshot: Snapshot }
export const defaultSettings = (): Settings => ({ limitKib: 0, closeToTray: false, autoCheck: false, backgroundCompletionNotice: true })
export const builtinRoutes = [{ id: '', name: '自动选择' }, { id: 'github', name: 'GitHub 直连' }, { id: 'gh-proxy', name: 'GH-Proxy' }, { id: 'ghproxy-net', name: 'ghproxy.net' }]
