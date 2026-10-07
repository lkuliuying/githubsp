<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue'
import { PhCloudArrowDown, PhDownloadSimple, PhClockCounterClockwise, PhStar, PhGearSix, PhTray, PhCheckCircle, PhX, PhWarningCircle, PhPlus, PhLink, PhFolderOpen, PhQueue, PhPlay, PhPause, PhTrash } from '@phosphor-icons/vue'
import TaskRow from './components/TaskRow.vue'
import SourcePicker from './components/SourcePicker.vue'
import RouteDiagnostics from './components/RouteDiagnostics.vue'
import HistoryView from './components/HistoryView.vue'
import SettingsView from './components/SettingsView.vue'
import FavoritesView from './components/FavoritesView.vue'
import UpdateView from './components/UpdateView.vue'
import { useDownloads } from './composables/useDownloads'
import { downloadsApi, errorMessage } from './services/downloads'
import { noticeApi } from './services/notices'
import type { TaskAction, DownloadTask } from './types'

const { snapshot, available, loading, error, pending, create, act, openDirectory, refresh, apply, perform } = useDownloads()
const picker = ref<InstanceType<typeof SourcePicker>>()
const view = ref<'downloads' | 'history' | 'favorites' | 'settings'>('downloads')
let stopNoticeNavigation: (() => void) | undefined
onMounted(async () => {
  if (!noticeApi.available()) return
  try {
    const stop = await noticeApi.subscribeNavigation(() => { if (!disposed) view.value = 'downloads' })
    if (disposed) stop()
    else stopNoticeNavigation = stop
  } catch (cause) { if (!disposed) error.value = errorMessage(cause) }
})
onUnmounted(() => stopNoticeNavigation?.())
const navigation = [
  { id: 'downloads', label: '下载', icon: PhDownloadSimple },
  { id: 'history', label: '历史', icon: PhClockCounterClockwise },
  { id: 'favorites', label: '收藏', icon: PhStar },
  { id: 'settings', label: '设置', icon: PhGearSix },
] as const
const queued = computed(() => snapshot.value.tasks.filter(task => task.status === 'queued'))
const url = ref('')
const directory = ref('')
const selecting = ref(false)
const submitting = ref(false)
const directorySource = ref<'input' | 'selected'>('input')
const preparingDirectory = ref(false)
const directoryDialog = ref<HTMLDialogElement>()
const directoryToCreate = ref<string | null>(null)
let resolveDirectoryConfirmation: ((confirmed: boolean) => void) | undefined
let inputRevision = 0
const dialog = ref<HTMLDialogElement>()
const cancelId = ref<string | null>(null)
const confirmation = ref<'cancel' | 'clear'>('cancel')
const batchAction = ref(false)
const showHint = ref(true)
let disposed = false
onUnmounted(() => { disposed = true; finishDirectoryConfirmation(false) })
const directoryTouched = ref(false)
const activeStates = ['probing', 'downloading', 'retrying', 'verifying', 'pausing', 'cancelling']
const activeCount = computed(() => snapshot.value.tasks.filter(task => activeStates.includes(task.status)).length)
const completedCount = computed(() => snapshot.value.tasks.filter(task => task.status === 'completed').length)
const resumable = computed(() => snapshot.value.tasks.filter(task => ['paused', 'failed'].includes(task.status)))
const pausable = computed(() => snapshot.value.tasks.filter(task => ['queued', 'waiting_network', 'probing', 'downloading', 'retrying', 'verifying'].includes(task.status)))
const ready = computed(() => available && !loading.value && !snapshot.value.error && snapshot.value.revision >= 0)
const directoryBusy = computed(() => selecting.value || submitting.value || preparingDirectory.value || pending.value.has('create'))
const canCreate = computed(() => ready.value && !!url.value.trim() && !!directory.value.trim() && !directoryBusy.value)
watch(() => snapshot.value.lastDirectory, value => { if (!directoryTouched.value && value) directory.value = value })
watch([directory, url, view, directorySource, ready], () => {
  inputRevision++
  finishDirectoryConfirmation(false)
}, { flush: 'sync' })

function finishDirectoryConfirmation(confirmed: boolean) {
  const resolve = resolveDirectoryConfirmation
  resolveDirectoryConfirmation = undefined
  directoryToCreate.value = null
  if (directoryDialog.value?.open) directoryDialog.value.close()
  if (resolve) void nextTick(() => {
    if (!disposed && ready.value && view.value === 'downloads') document.getElementById('save-directory')?.focus()
  })
  resolve?.(confirmed)
}

async function prepareDirectory(input: string, isCurrent: () => boolean): Promise<string | null> {
  if (preparingDirectory.value || selecting.value || !ready.value || disposed) return null
  const revision = inputRevision
  const current = () => !disposed && ready.value && input.trim() === directory.value.trim() && revision === inputRevision && isCurrent()
  preparingDirectory.value = true
  error.value = ''
  try {
    const inspected = await downloadsApi.inspectDirectory(input.trim())
    if (!current()) return null
    if (inspected.state === 'existing') return inspected.directory
    if (directorySource.value === 'selected') throw new Error('所选目录已不存在，请重新选择目录')
    directoryToCreate.value = inspected.directory
    const confirmed = new Promise<boolean>(resolve => { resolveDirectoryConfirmation = resolve })
    await nextTick()
    if (!current() || !directoryDialog.value) return null
    directoryDialog.value.showModal()
    if (!await confirmed || !current()) return null
    const created = await downloadsApi.createDirectory(inspected.directory)
    return current() ? created : null
  } finally {
    finishDirectoryConfirmation(false)
    preparingDirectory.value = false
  }
}

function directoryEdited() {
  directoryTouched.value = true
  directorySource.value = 'input'
}

async function selectDirectory() {
  if (directoryBusy.value || !ready.value) return
  const revision = inputRevision
  selecting.value = true
  error.value = ''
  try {
    const chosen = await downloadsApi.chooseDirectory(directory.value)
    if (!disposed && revision === inputRevision && chosen) {
      directory.value = chosen; directoryTouched.value = true; directorySource.value = 'selected'
    }
  } catch (cause) { if (!disposed && revision === inputRevision) error.value = errorMessage(cause) }
  finally { selecting.value = false }
}
async function submit() {
  if (!canCreate.value) return
  const source = url.value.trim(), revision = inputRevision
  submitting.value = true
  try {
    if (!/^https:\/\/github\.com\/[^/]+\/[^/]+\/releases\/download\//.test(source)) {
      await picker.value?.open(source)
      return
    }
    const target = await prepareDirectory(directory.value, () => revision === inputRevision)
    if (!target || disposed || revision !== inputRevision) return
    if (await create(source, target) && !disposed && revision === inputRevision) url.value = ''
  } catch (cause) {
    if (!disposed && revision === inputRevision) error.value = errorMessage(cause)
  } finally {
    submitting.value = false
  }
}
async function redownload(task: DownloadTask) {
  if (!ready.value || directoryBusy.value) return
  selecting.value = true
  try { const chosen = await downloadsApi.chooseDirectory(task.directory); if (chosen && !disposed && ready.value) await create(task.url, chosen) }
  catch (cause) { if (!disposed) error.value = errorMessage(cause) }
  finally { selecting.value = false }
}
async function diagnose() { await perform('diagnose', () => downloadsApi.diagnose(url.value)) }
async function hide() { await perform('hide', () => downloadsApi.hide()) }
async function acknowledge() { await perform('notices', () => downloadsApi.acknowledge()) }
async function favorite(repository: string) { await perform('favorite', () => downloadsApi.favorite('https://github.com/' + repository)) }
async function browseFavorite(repository: string) {
  view.value = 'downloads'
  await nextTick()
  await picker.value?.open('https://github.com/' + repository)
  document.getElementById('source-title')?.scrollIntoView({ block: 'start' })
}
async function move(id: string, direction: 'up' | 'down' | 'top') {
  const ids = queued.value.map(task => task.id), index = ids.indexOf(id)
  if (index < 0) return
  const target = direction === 'top' ? 0 : direction === 'up' ? Math.max(0, index - 1) : Math.min(ids.length - 1, index + 1)
  ids.splice(index, 1); ids.splice(target, 0, id)
  if (!await perform('order', () => downloadsApi.reorder(ids, snapshot.value.queueRevision))) await refresh()
}
async function requestAction(id: string, action: TaskAction) {
  if (action === 'cancel') {
    cancelId.value = id; confirmation.value = 'cancel'
    await nextTick(); dialog.value?.showModal()
  } else await act(id, action)
}
async function runBatch(action: 'pause' | 'resume' | 'remove') {
  if (batchAction.value || !ready.value) return
  batchAction.value = true
  const statuses = action === 'pause' ? ['queued', 'waiting_network', 'probing', 'downloading', 'retrying', 'verifying'] : action === 'resume' ? ['paused', 'failed'] : ['completed']
  const ids = snapshot.value.tasks.filter(task => statuses.includes(task.status)).map(task => task.id)
  try {
    for (const id of ids) {
      if (disposed) break
      const current = snapshot.value.tasks.find(task => task.id === id)
      if (!current || !statuses.includes(current.status) || pending.value.has(id)) continue
      if (!await act(id, action)) break
    }
  } finally { batchAction.value = false }
}
async function requestClear() { confirmation.value = 'clear'; await nextTick(); dialog.value?.showModal() }
async function confirmAction() {
  const id = cancelId.value
  dialog.value?.close(); cancelId.value = null
  if (confirmation.value === 'clear') await runBatch('remove')
  else if (id) await act(id, 'cancel')
}
</script>

<template>
  <div class="app-shell">
    <header class="app-header">
      <div class="app-brand"><PhCloudArrowDown :size="44" weight="fill" /><div><strong>GitHubSP</strong><span>更快的 GitHub 资源下载工具</span></div></div>
      <nav class="app-nav" aria-label="工作台页面">
        <button v-for="item in navigation" :key="item.id" :aria-current="view === item.id ? 'page' : undefined" @click="view = item.id">
          <component :is="item.icon" :size="23" :weight="view === item.id ? 'fill' : 'regular'" />{{ item.label }}
        </button>
      </nav>
      <button class="pa-btn app-tray" :disabled="!ready || pending.has('hide')" @click="hide"><PhTray :size="19" />收起到托盘</button>
    </header>
    <main>
      <div v-if="showHint" class="app-hintbar">
        <PhCheckCircle :size="23" weight="fill" /><span><strong>提示：</strong>支持 GitHub Release 附件和仓库地址，开始下载时自动检测可用线路。</span>
        <button class="pa-btn pa-btn--icon" aria-label="关闭使用提示" @click="showHint = false"><PhX :size="17" /></button>
      </div>
      <div v-if="!available" class="pa-message app-message"><PhWarningCircle :size="20" /><span>当前是浏览器预览。请打开 GitHubSP 桌面程序，使用目录选择和真实下载。</span></div>
      <div v-if="snapshot.error || error" class="pa-message pa-message--error app-message" role="alert"><PhWarningCircle :size="20" /><span>{{ snapshot.error || error }}</span><button v-if="!snapshot.error" class="pa-btn pa-btn--icon" aria-label="关闭错误提示" @click="error = ''"><PhX :size="16" /></button></div>
      <div v-if="snapshot.notices.length" class="pa-message pa-message--success app-message" role="status"><details><summary>{{ snapshot.notices.length }} 条新提醒 · {{ snapshot.notices.at(-1)?.message }}</summary><ul><li v-for="notice in snapshot.notices" :key="notice.id">{{ notice.message }}</li></ul></details><button class="pa-btn" :disabled="pending.has('notices')" @click="acknowledge">全部已读</button></div>

      <div v-show="view === 'downloads'" class="app-downloads">
        <section class="pa-panel app-composer" aria-labelledby="new-download-title">
          <div class="section-heading"><span class="section-heading__icon"><PhPlus :size="24" weight="bold" /></span><div class="section-heading__body"><h1 id="new-download-title">新建下载</h1><p>粘贴 Release 附件链接或仓库地址，输入或选择保存目录即可添加下载任务。</p></div></div>
          <form @submit.prevent="submit">
            <div class="pa-input-group app-source"><PhLink :size="21" /><label class="pa-sr-only" for="resource-url">GitHub 资源链接</label><input id="resource-url" v-model="url" class="pa-input" type="url" placeholder="请输入 GitHub 链接，例如 https://github.com/owner/repo/releases/latest" autocomplete="off" spellcheck="false" :disabled="!ready" required /></div>
            <div class="pa-input-group app-directory__control"><PhFolderOpen :size="21" /><label class="pa-sr-only" for="save-directory">保存到</label><input id="save-directory" v-model="directory" class="pa-input" placeholder="输入或选择保存目录" :disabled="!ready" required @input="directoryEdited" /><button type="button" class="pa-btn" :disabled="!ready || directoryBusy" @click="selectDirectory">{{ selecting ? '选择中…' : '浏览' }}</button></div>
            <button class="pa-btn pa-btn--primary app-submit" type="submit" :disabled="!canCreate"><PhPlus :size="20" weight="bold" />{{ submitting ? '正在添加…' : '添加下载任务' }}</button>
          </form>
        </section>
        <div class="app-workbench">
          <SourcePicker ref="picker" :directory="directory" :ready="ready" :prepare-directory="prepareDirectory" :directory-busy="directoryBusy" @snapshot="apply" @resume="id => act(id, 'resume')" @favorite="favorite" />
          <RouteDiagnostics :reports="snapshot.diagnostics" :context="snapshot.diagnosticContext" :diagnosing="snapshot.diagnosing || pending.has('diagnose')" :ready="ready" :has-active="activeCount > 0" :url="url" @diagnose="diagnose" />
        </div>
        <section class="pa-panel app-queue" aria-labelledby="queue-title" :aria-busy="loading">
          <div class="section-heading app-queue__heading">
            <span class="section-heading__icon"><PhQueue :size="23" /></span><div class="section-heading__body"><h2 id="queue-title">下载队列</h2><p>{{ snapshot.tasks.length }} 个任务 · {{ activeCount }} 个传输中 · {{ queued.length }} 个排队中 · {{ snapshot.tasks.filter(task => task.status === 'waiting_network').length }} 个等待线路恢复</p></div>
            <div class="app-queue__toolbar">
              <button class="pa-btn" :disabled="!ready || batchAction || !resumable.length" @click="runBatch('resume')"><PhPlay :size="16" weight="fill" />全部继续</button>
              <button class="pa-btn" :disabled="!ready || batchAction || !pausable.length" @click="runBatch('pause')"><PhPause :size="16" weight="fill" />全部暂停</button>
              <button class="pa-btn" :disabled="!ready || batchAction || !completedCount" @click="requestClear"><PhTrash :size="17" />清空已完成</button>
            </div>
          </div>
          <div v-if="loading" class="pa-empty" role="status"><PhCloudArrowDown :size="40" /><p>正在读取本地下载任务…</p></div>
          <div v-else-if="available && snapshot.revision < 0" class="pa-empty"><PhWarningCircle :size="36" /><h3>任务管理器暂未就绪</h3><button class="pa-btn" @click="refresh">重新连接</button></div>
          <div v-else class="pa-table-scroll" tabindex="0" aria-label="下载队列表格">
            <table class="pa-table app-task-table">
              <thead><tr><th scope="col">#</th><th scope="col">文件名</th><th scope="col">大小</th><th scope="col">进度</th><th scope="col">速度</th><th scope="col">剩余时间</th><th scope="col">线路</th><th scope="col">状态</th><th scope="col">操作</th></tr></thead>
              <TaskRow v-for="(task, index) in snapshot.tasks" :key="task.id" :task="task" :index="index + 1" :busy="pending.has(task.id) || !ready || batchAction" :order-busy="pending.has('order')" :first="queued[0]?.id === task.id" :last="queued.at(-1)?.id === task.id" @action="requestAction" @open="openDirectory" @snapshot="apply" @redownload="redownload" @favorite="favorite" @move="move" />
              <tbody v-if="!snapshot.tasks.length"><tr><td colspan="9"><div class="pa-empty"><PhCloudArrowDown :size="42" weight="light" /><h3>准备好你的第一个下载</h3><p>粘贴附件链接，选择保存位置，即可开始。</p></div></td></tr></tbody>
            </table>
          </div>
        </section>
      </div>
      <HistoryView v-if="view === 'history'" :ready="ready" @snapshot="apply" @redownload="redownload" />
      <FavoritesView v-if="view === 'favorites'" :favorites="snapshot.favorites" :ready="ready" @snapshot="apply" @browse="browseFavorite" />
      <SettingsView v-if="view === 'settings'" :settings="snapshot.settings" :ready="ready" @snapshot="apply"><UpdateView :ready="ready" /></SettingsView>
    </main>
    <footer class="app-footer"><span>GitHubSP <strong>v0.2.3</strong><i />简单 · 稳定 · 专注下载</span><span><PhClockCounterClockwise :size="15" />逐个下载 · 关闭时保存进度 · 重启后手动继续</span></footer>
    <dialog ref="dialog" class="app-dialog" @close="cancelId = null">
      <h2>{{ confirmation === 'clear' ? '清空已完成的记录？' : '取消这个下载？' }}</h2>
      <p>{{ confirmation === 'clear' ? '仅移除已完成任务的记录，下载文件会保留在原目录。' : '将停止下载并删除该任务的临时分片。已完成的文件不受影响。' }}</p>
      <p v-if="confirmation === 'cancel'" class="pa-muted">如果之后还要继续，请选择“暂停下载”。</p>
      <div class="app-dialog__actions"><button class="pa-btn" autofocus @click="dialog?.close()">保留记录</button><button class="pa-btn pa-btn--danger" @click="confirmAction">{{ confirmation === 'clear' ? '清空记录，保留文件' : '取消并清理' }}</button></div>
    </dialog>
    <dialog ref="directoryDialog" class="app-dialog app-directory-dialog" aria-labelledby="create-directory-title" aria-describedby="create-directory-description" @cancel.prevent="finishDirectoryConfirmation(false)" @close="!directoryDialog?.open && finishDirectoryConfirmation(false)">
      <h2 id="create-directory-title">当前目录不存在，是否新建目录？</h2>
      <p class="app-directory-dialog__path">{{ directoryToCreate }}</p>
      <p id="create-directory-description">将创建该目录及缺失的父目录，然后继续刚才的操作。</p>
      <div class="app-dialog__actions"><button class="pa-btn" autofocus @click="finishDirectoryConfirmation(false)">取消</button><button class="pa-btn pa-btn--primary" @click="finishDirectoryConfirmation(true)">新建并继续</button></div>
    </dialog>
  </div>
</template>

<style scoped>
.app-shell { min-height: 100vh; max-width: 1680px; margin: auto; display: flex; flex-direction: column; }
.app-header { min-height: 82px; display: grid; grid-template-columns: 1fr auto 1fr; align-items: center; gap: 24px; padding: 0 26px; background: #fbfdff; border-bottom: 1px solid var(--border); }
.app-brand { display: flex; align-items: center; gap: 12px; }
.app-brand > svg { color: var(--green); flex-shrink: 0; }
.app-brand strong { display: block; font-size: 22px; letter-spacing: -.5px; }
.app-brand span { display: block; font-size: 11px; color: var(--muted); margin-top: 3px; }
.app-nav { display: flex; align-items: stretch; gap: 7px; }
.app-nav button { position: relative; display: flex; gap: 10px; align-items: center; justify-content: center; border: 0; border-radius: 10px; background: transparent; color: var(--text); font-size: 15px; padding: 14px 26px; }
.app-nav button:hover { background: var(--surface-subtle); }
.app-nav button[aria-current='page'] { background: var(--green-soft); color: #00884c; font-weight: 650; }
.app-nav button[aria-current='page']::after { content: ''; position: absolute; bottom: 0; left: 12px; right: 12px; height: 3px; background: var(--green); border-radius: 2px; }
.app-tray { justify-self: end; background: transparent; border-color: transparent; }
main { flex: 1; min-width: 0; padding: 14px 20px 22px; }
.app-hintbar { display: flex; gap: 10px; align-items: center; padding: 7px 12px; min-height: 42px; margin-bottom: 14px; background: #e6f7ef; color: #416e69; border-radius: 10px; font-size: 12px; line-height: 1.6; }
.app-hintbar > svg, .app-hintbar strong { color: #048a4d; flex-shrink: 0; }
.app-hintbar > span { flex: 1; }
.app-hintbar .pa-btn { color: #416e69; }
.app-message { margin-bottom: 14px; align-items: center; }
.app-message > span, .app-message > details { flex: 1; min-width: 0; }
.app-downloads { display: flex; flex-direction: column; gap: 16px; }
.app-composer { padding: 16px; }
.app-composer .section-heading { margin-bottom: 14px; }
.app-composer .section-heading__body { display: flex; gap: 16px; align-items: baseline; }
.app-composer .section-heading p { margin: 0; }
.app-composer form { display: grid; grid-template-columns: minmax(200px, 1fr) minmax(250px, .44fr) auto; gap: 12px; }
.app-composer .pa-input, .app-submit { min-height: 46px; }
.app-submit { padding: 10px 22px; }
.app-workbench { display: grid; grid-template-columns: minmax(0, 1.7fr) minmax(340px, 1fr); gap: 16px; align-items: stretch; }
.app-queue { padding: 16px; }
.app-queue .pa-table-scroll { container-type: inline-size; }
.app-queue__heading .section-heading__body { display: flex; align-items: baseline; gap: 14px; }
.app-queue__heading p { margin: 0; }
.app-queue__toolbar { display: flex; gap: 8px; flex-wrap: wrap; }
.app-queue__toolbar .pa-btn { min-height: 36px; font-size: 12px; }
.app-task-table { min-width: 990px; }.app-task-table th { padding-top: 8px; padding-bottom: 8px; }.app-queue__heading { margin-bottom: 12px; }
.app-footer { display: flex; align-items: center; justify-content: space-between; flex-wrap: wrap; gap: 8px 20px; min-height: 43px; padding: 10px 26px; color: #7185a3; border-top: 1px solid var(--border); font-size: 11px; background: #f8fbfe; }
.app-footer > span { display: flex; align-items: center; gap: 8px; }
.app-footer strong { font-weight: 400; }.app-footer i { height: 12px; border-left: 1px solid var(--border-strong); margin: 0 4px; }
.app-dialog { width: 430px; max-width: calc(100vw - 32px); border: 1px solid var(--border); border-radius: 14px; padding: 26px; color: var(--text); box-shadow: 0 18px 60px #14294330; }
.app-dialog::backdrop { background: #17274766; }.app-dialog h2 { font-size: 19px; margin: 0 0 15px; }.app-dialog p { font-size: 13px; line-height: 1.8; }.app-dialog__actions { display: flex; justify-content: flex-end; gap: 10px; margin-top: 24px; flex-wrap: wrap; }
.app-directory-dialog__path { overflow-wrap: anywhere; max-height: 30vh; overflow-y: auto; }
@media (max-width: 1199px) { .app-header { gap: 12px; padding: 0 20px; }.app-brand strong { font-size: 20px; }.app-brand span { font-size: 10px; }.app-nav button { padding: 14px 17px; }.app-workbench { grid-template-columns: minmax(0, 1fr); }.app-queue__heading { flex-wrap: wrap; }.app-queue__heading .section-heading__body { display: block; }.app-composer .section-heading__body { display: block; }.app-composer .section-heading p { margin-top: 3px; } }
@media (max-width: 800px) { .app-header { grid-template-columns: 1fr auto; padding: 12px 16px 0; gap: 10px; }.app-nav { grid-column: 1 / -1; grid-row: 2; justify-content: center; }.app-nav button { flex: 1; padding: 12px; }.app-tray { grid-column: 2; grid-row: 1; }.app-brand span { font-size: 11px; }main { padding: 12px; }.app-composer form { grid-template-columns: minmax(0, 1fr) auto; }.app-source { grid-column: 1 / -1; }.app-queue__toolbar { width: 100%; }.app-footer { padding: 12px 16px; } }
@media (max-width: 480px) { .app-brand > svg { width: 35px; }.app-brand span { font-size: 10px; }.app-tray { font-size: 11px; padding: 6px; }.app-nav button { font-size: 13px; gap: 6px; }.app-composer form { grid-template-columns: minmax(0, 1fr); }.app-source { grid-column: auto; }.app-hintbar { align-items: flex-start; }.app-hintbar > svg { margin-top: 4px; }.app-queue__toolbar .pa-btn { flex: 1; padding: 7px; }}
</style>
