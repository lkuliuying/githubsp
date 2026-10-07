import { invoke, isTauri } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { open } from '@tauri-apps/plugin-dialog'
import type { Snapshot, TaskAction, TaskStatus, CatalogPage, BatchPreview, BatchResult, Settings, HistoryPage, UpdateResult } from '../types'

export const downloadsApi = {
  available: () => isTauri(),
  browse: (input: string, page = 0, includePrerelease = false) => invoke<CatalogPage>('browse_releases', { input, page, includePrerelease }),
  previewBatch: (urls: string[], directory: string) => invoke<BatchPreview>('preview_batch', { urls, directory }),
  createBatch: (urls: string[], directory: string, route: string | null) => invoke<BatchResult>('create_batch', { urls, directory, route }),
  changeRoute: (id: string, route: string | null, restart: boolean) => invoke<Snapshot>('change_route', { id, route, restart }),
  diagnose: (url: string) => invoke<Snapshot>('diagnose_routes', { url }),
  saveSettings: (settings: Settings) => invoke<Snapshot>('save_settings', { settings }),
  reorder: (ids: string[], revision: number) => invoke<Snapshot>('reorder_queue', { ids, revision }),
  history: (query: string, status: TaskStatus | null, page: number) => invoke<HistoryPage>('query_history', { query, status, page }),
  acknowledge: () => invoke<Snapshot>('acknowledge_notices'),
  hide: () => invoke<void>('hide_to_tray'),
  copy: (url: string) => navigator.clipboard.writeText(url),
  favorite: (input: string) => invoke<Snapshot>('add_favorite', { input }),
  removeFavorite: (repository: string) => invoke<Snapshot>('remove_favorite', { repository }),
  checkFavorites: (repository: string | null = null) => invoke<Snapshot>('check_favorites', { repository }),
  checkUpdate: () => invoke<UpdateResult>('check_app_update'),
  openRelease: (url: string) => invoke<void>('open_release', { url }),
  load: () => invoke<Snapshot>('list_tasks'),
  create: (url: string, directory: string) => invoke<Snapshot>('create_task', { url, directory }),
  act: (id: string, action: TaskAction) => invoke<Snapshot>('task_action', { id, action }),
  openDirectory: (id: string) => invoke<void>('open_directory', { id }),
  subscribe: (callback: (snapshot: Snapshot) => void) => listen<Snapshot>('downloads-changed', event => callback(event.payload)),
  chooseDirectory: async (current: string): Promise<string | null> => {
    const selected = await open({ directory: true, multiple: false, title: '选择下载保存目录', defaultPath: current || undefined });
    return typeof selected === 'string' ? selected : null
  },
}

export function errorMessage(error: unknown): string {
  if (typeof error === 'string') return error
  if (error instanceof Error) return error.message
  return '操作失败，请重试。'
}
