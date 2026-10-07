import type { TaskStatus } from './types'

export const statusLabels: Record<TaskStatus, string> = {
  queued: '等待下载', probing: '检测下载线路', downloading: '正在下载', retrying: '等待重试',
  verifying: '合并与校验', pausing: '正在保存进度', cancelling: '正在取消', paused: '已暂停',
  completed: '下载完成', failed: '下载失败', cancelled: '已取消',
}

export function formatBytes(value: number | null): string {
  if (value === null || !Number.isFinite(value) || value < 0) return '未知大小'
  if (value < 1024) return `${value} B`
  const units = ['KiB', 'MiB', 'GiB', 'TiB']
  let size = value / 1024
  let index = 0
  while (size >= 1024 && index < units.length - 1) { size /= 1024; index += 1 }
  return `${size.toFixed(size >= 100 ? 0 : 1)} ${units[index]}`
}

export function formatEta(seconds: number | null): string {
  if (seconds === null || !Number.isFinite(seconds)) return '计算中'
  if (seconds < 60) return `${Math.max(1, Math.ceil(seconds))} 秒`
  if (seconds < 3600) return `${Math.ceil(seconds / 60)} 分钟`
  return `${Math.floor(seconds / 3600)} 小时 ${Math.ceil(seconds % 3600 / 60)} 分钟`
}
