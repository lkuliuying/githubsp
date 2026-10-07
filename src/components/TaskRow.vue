<script setup lang="ts">
import { computed, ref } from 'vue'
import { PhFileArrowDown, PhPause, PhPlay, PhX, PhFolderOpen, PhTrash, PhArrowClockwise, PhDotsThreeVertical, PhCaretUp, PhCaretDown, PhArrowLineUp } from '@phosphor-icons/vue'
import type { DownloadTask, TaskAction, Snapshot } from '../types'
import { formatBytes, formatElapsed, formatEta, statusLabels, taskSpeed, taskFailure } from '../format'
import RouteControls from './RouteControls.vue'
import RouteFeedback from './RouteFeedback.vue'

const props = withDefaults(defineProps<{ task: DownloadTask; busy: boolean; index?: number; orderBusy?: boolean; first?: boolean; last?: boolean }>(), { index: 1, orderBusy: false, first: false, last: false })
const emit = defineEmits<{ action: [id: string, action: TaskAction]; open: [id: string]; snapshot: [snapshot: Snapshot]; redownload: [task: DownloadTask]; favorite: [repository: string]; move: [id: string, direction: 'up' | 'down' | 'top'] }>()
const expanded = ref(false)
const active = computed(() => ['probing', 'downloading', 'retrying', 'verifying'].includes(props.task.status))
const stopping = computed(() => ['pausing', 'cancelling'].includes(props.task.status))
const percent = computed(() => props.task.status === 'completed' ? 100 : props.task.total ? Math.min(100, props.task.downloaded / props.task.total * 100) : undefined)
const speed = computed(() => taskSpeed(props.task))
const failure = computed(() => taskFailure(props.task))
</script>

<template>
  <tbody class="task-row" :aria-label="task.filename">
    <tr>
      <td class="task-index">{{ index }}</td>
      <td class="task-name"><div><PhFileArrowDown :size="17" /><strong :title="task.filename">{{ task.filename }}</strong></div><p v-if="failure.message" class="pa-error">{{ failure.message }}</p></td>
      <td class="task-size">{{ formatBytes(task.total) }}</td>
      <td><div class="task-progress"><progress :value="percent" max="100" :aria-label="task.filename + ' 下载进度'" /><span>{{ percent === undefined ? '—' : percent.toFixed(0) + '%' }}</span></div></td>
      <td class="task-metric task-speed" :title="speed.title" :aria-label="speed.title + '：' + speed.text">{{ speed.text }}</td>
      <td class="task-metric">{{ task.status === 'downloading' ? formatEta(task.eta) : task.status === 'completed' ? '已完成' : '—' }}</td>
      <td class="task-route">{{ task.route || '—' }}</td>
      <td><span class="pa-badge" :class="{ 'pa-badge--success': task.status === 'completed' || task.status === 'downloading', 'pa-badge--error': task.status === 'failed', 'pa-badge--info': active && task.status !== 'downloading' }">{{ statusLabels[task.status] }}</span></td>
      <td><div class="task-actions">
        <button v-if="active || task.status === 'queued' || task.status === 'waiting_network'" class="pa-btn pa-btn--icon" :disabled="busy" title="暂停下载" aria-label="暂停下载" @click="emit('action', task.id, 'pause')"><PhPause :size="16" weight="fill" /></button>
        <button v-if="task.status === 'paused' || task.status === 'failed'" class="pa-btn pa-btn--icon" :disabled="busy" :title="task.status === 'failed' ? '重试下载' : '继续下载'" :aria-label="task.status === 'failed' ? '重试下载' : '继续下载'" @click="emit('action', task.id, 'resume')"><PhArrowClockwise v-if="task.status === 'failed'" :size="17" /><PhPlay v-else :size="16" weight="fill" /></button>
        <button v-if="!['completed', 'cancelled'].includes(task.status)" class="pa-btn pa-btn--icon" :disabled="busy || stopping" title="取消下载" aria-label="取消下载" @click="emit('action', task.id, 'cancel')"><PhX :size="16" /></button>
        <button v-if="task.status === 'completed'" class="pa-btn pa-btn--icon" :disabled="busy" title="打开所在目录" aria-label="打开所在目录" @click="emit('open', task.id)"><PhFolderOpen :size="18" /></button>
        <button class="pa-btn pa-btn--icon" title="下载详情与更多操作" :aria-label="'下载详情：' + task.filename" :aria-expanded="expanded" @click="expanded = !expanded"><PhDotsThreeVertical :size="18" weight="bold" /></button>
      </div></td>
    </tr>
    <tr v-if="task.status === 'waiting_network' || task.retryInfo || task.routeSuggestion" class="task-feedback-row"><td colspan="9"><RouteFeedback :task="task" :busy="busy" @snapshot="value => emit('snapshot', value)" @retry="emit('action', task.id, 'resume')" /></td></tr>
    <tr v-if="expanded" class="task-detail-row"><td colspan="9"><div class="task-details">
      <dl><dt>资源链接</dt><dd>{{ task.url }}</dd><dt>保存位置</dt><dd>{{ task.finalPath || task.directory }}</dd><dt>已下载</dt><dd>{{ formatBytes(task.downloaded) }} / {{ formatBytes(task.total) }}</dd><dt>{{ task.status === 'completed' ? '总耗时' : '已耗时' }}</dt><dd>{{ formatElapsed(task.elapsedMs) }}<span v-if="task.elapsedIsPartial">（记录不完整）</span><small class="task-elapsed-note">累计执行时间，不含排队、等待网络恢复、手动暂停和应用退出时间。</small></dd></dl>
      <RouteControls :task="task" :busy="busy" @snapshot="value => emit('snapshot', value)" @pause="id => emit('action', id, 'pause')" @redownload="value => emit('redownload', value)" @favorite="repository => emit('favorite', repository)" />
      <div class="task-detail-actions">
        <template v-if="task.status === 'queued'"><button class="pa-btn" :disabled="busy || orderBusy || first" @click="emit('move', task.id, 'up')"><PhCaretUp :size="16" />上移</button><button class="pa-btn" :disabled="busy || orderBusy || last" @click="emit('move', task.id, 'down')"><PhCaretDown :size="16" />下移</button><button class="pa-btn" :disabled="busy || orderBusy || first" @click="emit('move', task.id, 'top')"><PhArrowLineUp :size="16" />置顶</button></template>
        <button v-if="task.status === 'completed' || task.status === 'cancelled'" class="pa-btn pa-btn--danger" :disabled="busy" @click="emit('action', task.id, 'remove')"><PhTrash :size="16" />移除记录，保留文件</button>
      </div>
    </div></td></tr>
  </tbody>
</template>

<style scoped>
.task-elapsed-note { display: block; margin-top: 4px; color: var(--muted); font-size: 11px; }
.task-index { width: 30px; color: var(--muted); }.task-name { width: 24%; min-width: 180px; max-width: 310px; }.task-name > div { display: flex; align-items: center; gap: 8px; }.task-name svg { flex-shrink: 0; color: #6684a0; }.task-name strong { font-weight: 500; font-size: 12px; overflow-wrap: anywhere; }.task-name p { margin: 5px 0 0; font-size: 10px; max-width: 310px; }.task-size, .task-metric { white-space: nowrap; color: #566c8c; font-size: 11px; }.task-route { font-size: 11px; color: #526785; min-width: 85px; }
.task-progress { display: flex; align-items: center; gap: 8px; min-width: 130px; color: #637696; font-size: 10px; font-variant-numeric: tabular-nums; }.task-progress progress { width: 100px; height: 7px; flex: 1; border: 0; border-radius: 5px; overflow: hidden; accent-color: var(--green); background: #dfe7ef; }progress::-webkit-progress-bar { background: #dfe7ef; }progress::-webkit-progress-value { background: #16b36a; border-radius: 5px; }
.task-actions { display: flex; gap: 1px; }.task-actions .pa-btn { width: 27px; min-height: 28px; padding: 4px; }
.task-detail-row > td { background: #f8fbff !important; padding: 18px 22px; }.task-details { max-width: 950px; }dl { display: grid; grid-template-columns: 70px minmax(0, 1fr); gap: 10px; font-size: 12px; margin: 0 0 18px; }dt { color: var(--muted); }dd { margin: 0; overflow-wrap: anywhere; user-select: text; }.task-detail-actions { display: flex; gap: 8px; margin-top: 12px; flex-wrap: wrap; }
.task-row > tr > td { padding-top: 3px; padding-bottom: 3px; }.task-row .task-detail-row > td { padding: 18px 22px; }.task-size, .task-metric, .task-route { font-size: 12px; }.task-progress { min-width: 120px; }.task-name { min-width: 190px; }.task-name strong { font-size: 12px; }.task-actions .pa-btn { min-height: 26px; }
</style>
