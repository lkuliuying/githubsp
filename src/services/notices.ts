import { invoke, isTauri } from '@tauri-apps/api/core'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { listen } from '@tauri-apps/api/event'
import type { DownloadNoticeView } from '../types'

export const noticeApi = {
  available: () => isTauri(),
  windowLabel: () => isTauri() ? getCurrentWindow().label : 'main',
  read: () => invoke<DownloadNoticeView>('read_download_notice'),
  presented: (revision: number) => invoke<void>('present_download_notice', { revision }),
  dismiss: (revision: number) => invoke<void>('dismiss_download_notice', { revision }),
  hover: (revision: number, hovering: boolean) => invoke<void>('hover_download_notice', { revision, hovering }),
  open: (revision: number) => invoke<void>('open_download_notice', { revision }),
  subscribe: (callback: (view: DownloadNoticeView) => void) => listen<DownloadNoticeView>('download-notice-changed', event => callback(event.payload)),
  subscribeNavigation: (callback: () => void) => listen('show-downloads', callback),
}
