import type { DownloadTask, TaskStatus } from './types'

export const statusLabels: Record<TaskStatus, string> = {
  queued: '等待下载', probing: '检测下载线路', downloading: '正在下载', retrying: '等待重试', waiting_network: '等待线路恢复',
  verifying: '正在完成下载', pausing: '正在保存进度', cancelling: '正在取消', paused: '已暂停',
  completed: '下载完成', failed: '下载失败', cancelled: '已取消',
}

export function formatBytes(value: number | null): string {
  if (value === null || !Number.isFinite(value) || value < 0) return '未知大小'
  const units = ['B', 'KB', 'MB', 'GB', 'TB']
  let size = value
  let index = 0
  while (size >= 1000 && index < units.length - 1) { size /= 1000; index += 1 }
  if (Number(size.toFixed(index === 0 || size >= 100 ? 0 : 1)) >= 1000 && index < units.length - 1) { size /= 1000; index += 1 }
  return `${size.toFixed(index === 0 || size >= 100 ? 0 : 1)} ${units[index]}`
}

export function taskSpeed(task: DownloadTask): { text: string; title: string } {
  if (task.status === 'downloading') {
    return Number.isFinite(task.speed) && task.speed >= 0
      ? { text: `${formatBytes(task.speed)}/s`, title: '实时下载速度' }
      : { text: '—', title: '暂无实时速度' }
  }
  if (task.status !== 'completed') return { text: '—', title: '当前未在传输' }
  if (task.elapsedIsPartial) return { text: '—', title: '耗时记录不完整，无法计算平均速度' }
  if (task.elapsedMs == null || !Number.isFinite(task.elapsedMs) || task.elapsedMs <= 0) {
    return { text: '—', title: '缺少有效耗时，无法计算平均速度' }
  }
  const size = task.total ?? task.downloaded
  const speed = size / task.elapsedMs * 1000
  if (!Number.isFinite(size) || size < 0 || !Number.isFinite(speed)) {
    return { text: '—', title: '缺少有效文件大小，无法计算平均速度' }
  }
  return { text: `平均 ${formatBytes(speed)}/s`, title: '最终文件大小 ÷ 累计执行耗时，不含排队、等待网络恢复、手动暂停和应用退出时间' }
}

export function taskFailure(task: DownloadTask): { message: string | null; action: string | null } {
  const integrity = task.failure?.category === 'integrity' || task.verification === 'failed'
  return {
    message: task.error && integrity ? '下载文件内容异常，请重新下载。' : task.routeFailures?.length ? task.error?.split('。')[0] ?? null : task.error,
    action: task.failure && integrity ? '请切换线路重新下载；不要运行不完整文件。' : task.failure?.action ?? null,
  }
}

export function formatEta(seconds: number | null): string {
  if (seconds === null || !Number.isFinite(seconds)) return '计算中'
  if (seconds < 60) return `${Math.max(1, Math.ceil(seconds))} 秒`
  if (seconds < 3600) return `${Math.ceil(seconds / 60)} 分钟`
  return `${Math.floor(seconds / 3600)} 小时 ${Math.ceil(seconds % 3600 / 60)} 分钟`
}

export function formatElapsed(milliseconds?: number | null): string {
  if (milliseconds == null || !Number.isFinite(milliseconds) || milliseconds < 0) return '未知'
  if (milliseconds < 1000) return '不足 1 秒'
  const seconds = Math.floor(milliseconds / 1000)
  if (seconds < 60) return `${seconds} 秒`
  if (seconds < 3600) return `${Math.floor(seconds / 60)} 分 ${seconds % 60} 秒`
  return `${Math.floor(seconds / 3600)} 小时 ${String(Math.floor(seconds % 3600 / 60)).padStart(2, '0')} 分 ${String(seconds % 60).padStart(2, '0')} 秒`
}
