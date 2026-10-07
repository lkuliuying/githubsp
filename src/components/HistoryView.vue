<script setup lang="ts">
import { computed, onUnmounted, ref, watch } from 'vue'
import { PhClockCounterClockwise, PhMagnifyingGlass, PhArrowCounterClockwise, PhFiles, PhCheckCircle, PhXCircle, PhWarningCircle, PhListBullets, PhFile, PhLink, PhDownloadSimple, PhFolderOpen, PhTrash, PhCaretLeft, PhCaretRight, PhSortDescending } from '@phosphor-icons/vue'
import type { DownloadTask, HistoryPage, Snapshot, TaskStatus } from '../types'
import { downloadsApi, errorMessage } from '../services/downloads'
import { statusLabels } from '../format'

const emit = defineEmits<{ snapshot: [value: Snapshot]; redownload: [task: DownloadTask] }>()
const props = defineProps<{ ready: boolean }>()
const query = ref(''), status = ref<TaskStatus | ''>(''), page = ref(1), jumpPage = ref(1)
const data = ref<HistoryPage | null>(null), busy = ref(false), error = ref(''), message = ref('')
const pageCount = computed(() => Math.max(1, Math.ceil((data.value?.total ?? 0) / (data.value?.pageSize || 20))))
const pageNumbers = computed(() => {
  const start = Math.max(1, Math.min(page.value - 2, pageCount.value - 4))
  return Array.from({ length: Math.min(5, pageCount.value) }, (_, index) => start + index)
})
const completed = computed(() => data.value?.items.filter(entry => entry.task.status === 'completed').length ?? 0)
const failed = computed(() => data.value?.items.filter(entry => entry.task.status === 'failed').length ?? 0)
const missing = computed(() => data.value?.items.filter(entry => ['missing', 'inaccessible'].includes(entry.fileState)).length ?? 0)
let disposed = false, generation = 0
onUnmounted(() => { disposed = true; generation++ })
async function load(reset = false) {
  if (!props.ready) return
  if (reset) page.value = 1
  const current = ++generation; busy.value = true; error.value = ''
  try {
    const value = await downloadsApi.history(query.value, status.value || null, page.value)
    if (disposed || current !== generation) return
    const lastPage = Math.max(1, Math.ceil(value.total / value.pageSize))
    if (page.value > lastPage) { page.value = lastPage; await load(); return }
    data.value = value; page.value = value.page; jumpPage.value = value.page
  } catch (cause) { if (!disposed && current === generation) error.value = errorMessage(cause) }
  finally { if (!disposed && current === generation) busy.value = false }
}
watch(() => props.ready, ready => { if (ready) void load() }, { immediate: true })
async function action(task: DownloadTask, name: 'open' | 'copy' | 'remove') {
  if (busy.value || !props.ready) return
  busy.value = true; error.value = ''; message.value = ''
  try {
    if (name === 'open') await downloadsApi.openDirectory(task.id)
    if (name === 'copy') { await downloadsApi.copy(task.url); if (!disposed) message.value = '已复制来源链接' }
    if (name === 'remove') {
      const value = await downloadsApi.act(task.id, 'remove')
      if (!disposed) { emit('snapshot', value); await load() }
    }
  } catch (cause) { if (!disposed) error.value = errorMessage(cause) }
  finally { if (!disposed) busy.value = false }
}
async function reset() { query.value = ''; status.value = ''; await load(true) }
async function go(number: number) {
  if (busy.value || !props.ready) return
  if (!Number.isInteger(number) || number < 1 || number > pageCount.value) { error.value = '请输入 1 至 ' + pageCount.value + ' 之间的页码'; return }
  page.value = number; await load()
}
const date = (value?: number | null) => value ? new Date(value).toLocaleString() : '未知'
const fileLabels = { present: '正常', missing: '文件缺失', inaccessible: '路径不可访问', unfinished: '尚未完成' }
</script>

<template>
  <div class="history-view">
    <section class="pa-panel history-overview">
      <div class="section-heading section-heading--page"><span class="section-heading__icon"><PhClockCounterClockwise :size="29" /></span><div class="section-heading__body"><h1>下载历史</h1><p>搜索和管理过去的下载记录，快速查找、重新下载和定位文件。</p></div></div>
      <form class="history-filters" @submit.prevent="load(true)">
        <label class="history-search">文件名、仓库或版本<div class="pa-input-group"><PhMagnifyingGlass :size="19" /><input v-model="query" class="pa-input" maxlength="1024" placeholder="输入文件名、owner/repo 或版本关键词" :disabled="busy" /></div></label>
        <label>任务状态<select v-model="status" class="pa-input" :disabled="busy"><option value="">全部状态</option><option v-for="(label, key) in statusLabels" :key="key" :value="key">{{ label }}</option></select></label>
        <button class="pa-btn pa-btn--primary" :disabled="!ready || busy"><PhMagnifyingGlass :size="18" />{{ busy ? '读取中…' : '搜索 / 刷新' }}</button>
        <button type="button" class="pa-btn" :disabled="!ready || busy" @click="reset"><PhArrowCounterClockwise :size="18" />重置</button>
      </form>
      <div class="history-stats">
        <div class="pa-stat"><PhFiles :size="33" weight="duotone" /><div><span>匹配记录</span><strong>{{ data?.total ?? '—' }}</strong></div></div>
        <div class="pa-stat pa-stat--success"><PhCheckCircle :size="33" weight="fill" /><div><span>本页已完成</span><strong>{{ data ? completed : '—' }}</strong></div></div>
        <div class="pa-stat pa-stat--error"><PhXCircle :size="33" weight="fill" /><div><span>本页下载失败</span><strong>{{ data ? failed : '—' }}</strong></div></div>
        <div class="pa-stat pa-stat--warning"><PhWarningCircle :size="33" weight="fill" /><div><span>本页文件异常</span><strong>{{ data ? missing : '—' }}</strong></div></div>
      </div>
    </section>
    <section class="pa-panel history-records" :aria-busy="busy">
      <div class="section-heading"><span class="section-heading__icon"><PhListBullets :size="23" /></span><div class="section-heading__body"><h2>下载历史列表 <span>{{ data ? '共 ' + data.total + ' 条记录' : '' }}</span></h2></div><span class="history-sort"><PhSortDescending :size="18" />按添加时间倒序</span></div>
      <p v-if="error" role="alert" class="pa-message pa-message--error">{{ error }}</p><p v-if="message" role="status" class="pa-message pa-message--success">{{ message }}</p>
      <p v-if="busy" role="status" class="history-loading">正在读取…</p>
      <div class="pa-table-scroll" tabindex="0" aria-label="下载历史记录">
        <table class="pa-table history-table"><thead><tr><th scope="col">#</th><th scope="col">文件名</th><th scope="col">仓库 / 来源</th><th scope="col">版本</th><th scope="col">下载时间</th><th scope="col">保存路径</th><th scope="col">文件状态</th><th scope="col">操作</th></tr></thead>
          <tbody><tr v-for="(entry, index) in data?.items" :key="entry.task.id">
            <td class="pa-muted">{{ (page - 1) * (data?.pageSize || 20) + index + 1 }}</td>
            <td class="history-name"><div><PhFile :size="17" /><strong :title="entry.task.filename">{{ entry.task.filename }}</strong></div><small>{{ statusLabels[entry.task.status] }}</small></td>
            <td class="history-repository">{{ entry.task.repository || '未知仓库' }}</td>
            <td>{{ entry.task.tag || '未知版本' }}</td>
            <td class="history-date">{{ date(entry.task.createdAt) }}<small>完成：{{ date(entry.task.completedAt) }}</small></td>
            <td class="history-path"><span :title="entry.task.finalPath || entry.task.directory">{{ entry.task.finalPath || entry.task.directory }}</span></td>
            <td><span class="pa-badge" :class="entry.fileState === 'present' ? 'pa-badge--success' : entry.fileState === 'missing' ? 'pa-badge--error' : entry.fileState === 'inaccessible' ? 'pa-badge--warning' : ''">{{ fileLabels[entry.fileState] }}</span><span v-if="entry.fileState === 'missing'" class="pa-sr-only">文件已不在原位置</span><span v-if="entry.fileState === 'inaccessible'" class="pa-sr-only">无法访问文件，请检查目录权限或磁盘连接</span></td>
            <td><div class="history-actions"><button class="pa-btn pa-btn--icon" :disabled="busy || !ready" title="复制来源链接" @click="action(entry.task, 'copy')"><PhLink :size="17" /><span class="pa-sr-only">复制来源链接</span></button><button class="pa-btn pa-btn--icon" :disabled="busy || !ready" title="重新下载" @click="emit('redownload', entry.task)"><PhDownloadSimple :size="17" /><span class="pa-sr-only">重新下载</span></button><button class="pa-btn pa-btn--icon" :disabled="busy || !ready" title="打开目录" @click="action(entry.task, 'open')"><PhFolderOpen :size="18" /><span class="pa-sr-only">打开目录</span></button><button v-if="['completed', 'cancelled'].includes(entry.task.status)" class="pa-btn pa-btn--icon" :disabled="busy || !ready" title="移除记录，保留文件" @click="action(entry.task, 'remove')"><PhTrash :size="17" /><span class="pa-sr-only">移除记录，保留文件</span></button></div></td>
          </tr></tbody>
        </table>
      </div>
      <div v-if="!busy && !data?.items.length" class="pa-empty"><PhClockCounterClockwise :size="40" weight="light" /><h3>{{ ready ? '没有匹配的下载记录' : '等待连接桌面程序' }}</h3><p>{{ ready ? '尝试其他关键词，或回到下载页添加任务。' : '下载记录会在桌面程序中显示。' }}</p></div>
      <div v-if="data" class="history-pagination"><span>共 {{ data.total }} 条记录，每页 {{ data.pageSize }} 条</span><div class="history-pages"><button class="pa-btn" aria-label="上一页" :disabled="busy || page <= 1" @click="go(page - 1)"><PhCaretLeft :size="16" /></button><button v-for="number in pageNumbers" :key="number" class="pa-btn" :class="{ 'pa-btn--primary': page === number }" :aria-current="page === number ? 'page' : undefined" :disabled="busy" @click="go(number)">{{ number }}</button><button class="pa-btn" aria-label="下一页" :disabled="busy || page >= pageCount" @click="go(page + 1)"><PhCaretRight :size="16" /></button></div><form class="history-jump" @submit.prevent="go(jumpPage)"><label for="history-page">跳转到</label><input id="history-page" v-model.number="jumpPage" class="pa-input" type="number" min="1" :max="pageCount" :disabled="busy" /><span>/ {{ pageCount }} 页</span><button class="pa-btn" :disabled="busy">确定</button></form></div>
      <p class="history-note">打开、搜索或翻页时检查当前页文件。移除记录会保留下载文件。</p>
    </section>
  </div>
</template>

<style scoped>
.history-view { display: flex; flex-direction: column; gap: 16px; }.history-filters { display: flex; gap: 12px; align-items: flex-end; padding: 16px; margin: 18px 0 14px; border: 1px solid var(--border); border-radius: 9px; }.history-filters label { display: block; font-size: 12px; font-weight: 500; min-width: 155px; }.history-filters .pa-input, .history-filters .pa-input-group { margin-top: 8px; }.history-filters .pa-input-group .pa-input { margin: 0; }.history-search { flex: 1; }.history-filters > .pa-btn { min-height: 42px; padding: 10px 21px; }
.history-stats { display: grid; grid-template-columns: repeat(4, minmax(0, 1fr)); gap: 14px; }.history-records .section-heading h2 > span { font-size: 12px; font-weight: 400; color: var(--muted); margin-left: 12px; }.history-sort { display: flex; align-items: center; gap: 8px; color: #56708f; font-size: 12px; }
.history-table { min-width: 1000px; }.history-table td { font-size: 11px; }.history-name { width: 20%; max-width: 290px; }.history-name > div { display: flex; align-items: center; gap: 8px; }.history-name svg { flex-shrink: 0; }.history-name strong { font-weight: 500; overflow-wrap: anywhere; }.history-name small, .history-date small { display: block; color: var(--muted); font-size: 10px; margin-top: 4px; }.history-name small { margin-left: 25px; }.history-repository { max-width: 150px; overflow-wrap: anywhere; }.history-date { min-width: 115px; font-variant-numeric: tabular-nums; }.history-path { max-width: 175px; }.history-path > span { display: block; max-width: 175px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }.history-actions { display: flex; gap: 3px; }.history-actions .pa-btn { width: 27px; min-height: 28px; padding: 4px; }.history-loading { font-size: 12px; color: var(--muted); }
.history-pagination { display: flex; align-items: center; justify-content: space-between; gap: 14px; margin-top: 20px; flex-wrap: wrap; font-size: 11px; color: var(--muted); }.history-pages { display: flex; gap: 6px; }.history-pages .pa-btn { min-width: 33px; min-height: 34px; padding: 5px 9px; }.history-jump { display: flex; align-items: center; gap: 8px; }.history-jump input { width: 57px; min-height: 34px; padding: 5px 7px; }.history-jump button { min-height: 34px; font-size: 11px; }.history-note { color: var(--muted); font-size: 11px; margin: 16px 0 0; line-height: 1.7; }
@media (max-width: 900px) { .history-stats { gap: 10px; }.pa-stat { padding: 13px; gap: 9px; }.pa-stat svg { width: 25px; }.history-filters { flex-wrap: wrap; }.history-search { flex-basis: calc(100% - 190px); }.history-filters > .pa-btn { padding: 9px 18px; }.history-sort { font-size: 11px; } }
@media (max-width: 620px) { .history-stats { grid-template-columns: repeat(2, minmax(0, 1fr)); }.history-filters { padding: 12px; }.history-filters label { min-width: 0; flex-basis: 100%; }.history-records .section-heading { flex-wrap: wrap; }.history-records .section-heading h2 > span { display: block; margin: 3px 0 0; }.history-sort { margin-left: 50px; } }
.history-records > .pa-table-scroll { max-height: 418px; }.history-table th { position: sticky; top: 0; z-index: 1; }.history-table td { font-size: 12px; padding: 8px 10px; }
.history-overview { padding: 18px; }.history-filters { padding: 14px; }.history-stats .pa-stat { padding-top: 15px; padding-bottom: 15px; }
</style>
