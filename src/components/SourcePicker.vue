<script setup lang="ts">
import { computed, onUnmounted, ref, watch } from 'vue'
import { PhPackage, PhMagnifyingGlass, PhCaretRight, PhCaretLeft, PhPlus, PhStar, PhFiles, PhCheckCircle, PhWarningCircle } from '@phosphor-icons/vue'
import { downloadsApi, errorMessage } from '../services/downloads'
import { builtinRoutes, type BatchPreview, type CatalogPage, type Snapshot } from '../types'
import { formatBytes } from '../format'

const props = defineProps<{ directory: string; ready: boolean }>()
const emit = defineEmits<{ snapshot: [value: Snapshot]; resume: [id: string]; favorite: [repository: string] }>()
const input = ref(''), repositoryInput = ref(''), busy = ref(false), error = ref(''), includePrerelease = ref(false), route = ref('')
const page = ref<CatalogPage | null>(null), selected = ref<string[]>([]), preview = ref<BatchPreview | null>(null), result = ref('')
const releaseId = ref<number | null>(null)
const release = computed(() => page.value?.releases.find(item => item.id === releaseId.value) ?? page.value?.releases[0])
const valid = computed(() => preview.value?.items.filter(item => item.status === 'valid') ?? [])
let generation = 0
let disposed = false
onUnmounted(() => { disposed = true; generation++ })
watch([input, repositoryInput, () => props.directory], () => { preview.value = null; result.value = ''; generation++ }, { flush: 'sync' })
watch(repositoryInput, () => { page.value = null; selected.value = []; releaseId.value = null }, { flush: 'sync' })
async function run(work: (current: number) => Promise<void>) {
  if (busy.value || !props.ready) return
  const current = ++generation
  busy.value = true; error.value = ''
  try { await work(current) } catch (cause) { if (!disposed && current === generation) error.value = errorMessage(cause) }
  finally { if (!disposed) busy.value = false }
}
async function browse(number = 0, repository?: string) {
  await run(async current => {
    const source = repository ? 'https://github.com/' + repository : (repositoryInput.value || input.value).trim()
    const normalized = /^[a-zA-Z0-9_.-]+\/[a-zA-Z0-9_.-]+$/.test(source) ? 'https://github.com/' + source : source
    const value = await downloadsApi.browse(normalized, number, includePrerelease.value)
    if (current !== generation || disposed) return
    page.value = value
    selected.value = value.selectedUrl ? [value.selectedUrl] : []
    releaseId.value = value.releases.find(item => item.assets.some(asset => asset.url === value.selectedUrl))?.id ?? value.releases[0]?.id ?? null
    preview.value = null
  })
}
async function open(value: string) { repositoryInput.value = value; await browse() }
defineExpose({ open })
async function prepare(fromSelection: boolean) {
  const urls = fromSelection ? selected.value : input.value.split(/\r?\n/).map(value => value.trim()).filter(Boolean)
  await run(async current => {
    const value = await downloadsApi.previewBatch(urls, props.directory.trim())
    if (current === generation && !disposed) preview.value = value
  })
}
async function submit() {
  const urls = valid.value.map(item => item.url!).filter(Boolean)
  await run(async current => {
    const value = await downloadsApi.createBatch(urls, props.directory.trim(), route.value || null)
    if (disposed) return
    emit('snapshot', value.snapshot)
    if (current !== generation) return
    const failed = value.items.filter(item => item.status === 'failed')
    const invalid = preview.value?.items.filter(item => item.status === 'invalid') ?? []
    input.value = [...failed, ...invalid].map(item => item.input).join('\n')
    preview.value = null; selected.value = []
    result.value = '已创建 ' + (value.items.length - failed.length) + ' 个任务。' + (failed.length + invalid.length ? '未成功的链接已保留，可修改后重试。' : '')
    if (failed.length) error.value = failed.map(item => item.message).join('；')
  })
}
function selectRelease(id: number) { releaseId.value = id; preview.value = null }
</script>

<template>
  <section class="source-panel pa-panel" aria-labelledby="source-title">
    <div class="section-heading"><span class="section-heading__icon"><PhPackage :size="23" weight="fill" /></span><div class="section-heading__body"><h2 id="source-title">查找附件与批量添加</h2><p>浏览仓库版本，选择需要下载的附件，支持批量添加。</p></div></div>
    <p v-if="busy" class="source-feedback" role="status">正在处理，请稍候…</p>
    <p v-if="error" role="alert" class="pa-message pa-message--error source-feedback"><PhWarningCircle :size="18" />{{ error }}</p>
    <p v-if="result" role="status" class="pa-message pa-message--success source-feedback"><PhCheckCircle :size="18" />{{ result }}</p>
    <div class="source-browser">
      <div class="source-repository-column">
        <form class="source-search" @submit.prevent="browse()"><div class="pa-input-group"><PhMagnifyingGlass :size="19" /><label class="pa-sr-only" for="repository-source">仓库或 Release 页面</label><input id="repository-source" v-model="repositoryInput" class="pa-input" placeholder="owner/repo 或 GitHub 仓库链接" :disabled="!ready || busy" /><button class="pa-btn pa-btn--primary" :disabled="!ready || busy || !(repositoryInput || input).trim()" title="查找版本与附件"><PhMagnifyingGlass :size="17" /><span>查找版本</span></button></div></form>
      <div class="source-versions">
        <div v-if="!page" class="pa-empty source-empty"><PhPackage :size="34" weight="light" /><p>搜索项目<br />查看可下载的版本</p></div>
        <p v-else-if="!page.releases.length" class="source-no-release">本页没有符合条件的版本，可翻页或显示预发布。</p>
        <div v-else class="source-release-list">
          <button v-for="item in page.releases" :key="item.id" type="button" class="source-release" :aria-pressed="release?.id === item.id" :disabled="busy" @click="selectRelease(item.id)">
            <PhPackage :size="22" /><span><strong>{{ item.tag }}</strong><small>{{ item.prerelease ? '预发布' : '正式版' }} · {{ item.assets.length }} 个附件</small></span><PhCaretRight :size="15" />
          </button>
        </div>
        <div v-if="page" class="source-pagination">
          <button class="pa-btn pa-btn--icon" title="收藏项目" aria-label="收藏项目" :disabled="busy || !ready" @click="emit('favorite', page.repository)"><PhStar :size="17" /></button>
          <button v-if="page.page === 0" class="pa-btn" :disabled="busy" @click="browse(1, page.repository)">浏览其他版本</button>
          <template v-else><button class="pa-btn pa-btn--icon" aria-label="上一页版本" :disabled="busy || page.page <= 1" @click="browse(page.page - 1, page.repository)"><PhCaretLeft :size="16" /></button><span>第 {{ page.page }} 页</span><button class="pa-btn pa-btn--icon" aria-label="下一页版本" :disabled="busy || !page.hasMore" @click="browse(page.page + 1, page.repository)"><PhCaretRight :size="16" /></button></template>
        </div>
      </div>
      </div>
      <div class="source-assets">
        <div class="source-asset-heading"><div><strong>{{ release ? '发布版本：' + release.tag : '版本附件' }}</strong><p>{{ release ? release.assets.length + ' 个附件 · 请确认平台和架构' : '选择版本后，附件会显示在这里' }}</p></div><label class="source-check"><input v-model="includePrerelease" type="checkbox" :disabled="!ready || busy" @change="page && browse(1, page.repository)" />显示预发布</label></div>
        <div class="pa-table-scroll source-asset-scroll">
          <table class="pa-table"><thead><tr><th scope="col">选择</th><th scope="col">文件名</th><th scope="col">大小</th></tr></thead>
            <tbody><tr v-for="asset in release?.assets" :key="asset.id" class="asset"><td><input v-model="selected" type="checkbox" :value="asset.url" :aria-label="'选择 ' + asset.name" :disabled="busy || !ready || !!asset.unavailable || (!selected.includes(asset.url) && selected.length >= 100)" @change="preview = null" /></td><td><strong :title="asset.name">{{ asset.name }}</strong><small>{{ asset.hints.join(' · ') || '其他附件' }} · {{ asset.sha256 ? '有官方 SHA-256' : '缺少官方摘要' }}</small><small v-if="asset.unavailable" class="pa-error">{{ asset.unavailable }}</small></td><td>{{ formatBytes(asset.size) }}</td></tr>
              <tr v-if="!release?.assets.length"><td colspan="3"><div class="pa-empty source-empty"><PhFiles :size="32" weight="light" /><p>{{ release ? '此版本没有可下载附件' : '暂无附件，请先查找项目' }}</p></div></td></tr>
            </tbody>
          </table>
        </div>
        <div class="source-selection"><span>已选 {{ selected.length }} 项</span><button class="pa-btn pa-btn--soft" :disabled="busy || !ready || !selected.length || !directory.trim()" @click="prepare(true)">预览所选附件<PhCaretRight :size="14" /></button></div>
        <div class="source-batch"><label class="pa-sr-only" for="project-source">批量附件直链，每行一个</label><textarea id="project-source" v-model="input" class="pa-input" rows="2" :disabled="!ready || busy" placeholder="在此粘贴多个附件直链，每行一个…" /><button class="pa-btn pa-btn--primary" :disabled="!ready || busy || !input.trim() || !directory.trim()" @click="prepare(false)"><PhPlus :size="17" />预览批量链接</button></div>
      </div>
    </div>
    <p class="source-note">每批最多 100 项，保存到上方目录。GitHub 自动生成的源码包不属于 Release 附件。</p>
    <div v-if="preview" class="source-preview" aria-label="批量预览">
      <h3>提交前确认</h3><p>有效 {{ valid.length }} 项 · 重复 {{ preview.items.filter(item => item.status === 'duplicate').length }} 项 · 无效 {{ preview.items.filter(item => item.status === 'invalid').length }} 项</p>
      <p>已知总大小 {{ formatBytes(preview.knownSize) }} · 大小未知 {{ preview.unknownCount }} 项 · 可用空间 {{ formatBytes(preview.preflight.available) }}</p>
      <p class="pa-muted">{{ preview.preflight.warning || '已按分片与合并文件检查空间。' }} 目标：{{ preview.preflight.directory }}</p>
      <ul><li v-for="(item, index) in preview.items" :key="index"><span class="pa-badge" :class="item.status === 'valid' ? 'pa-badge--success' : 'pa-badge--warning'">{{ item.status === 'valid' ? '有效' : item.status === 'duplicate' ? '重复' : '无效' }}</span> {{ item.filename || item.input }} <span>{{ item.message }}</span><button v-if="item.taskId" class="pa-btn" :disabled="busy" @click="emit('resume', item.taskId)">继续原任务</button></li></ul>
      <div class="source-preview-actions"><label>下载线路 <select v-model="route" class="pa-input" :disabled="busy"><option v-for="item in builtinRoutes" :key="item.id" :value="item.id">{{ item.name }}</option></select></label><button class="pa-btn pa-btn--primary" :disabled="busy || !valid.length || !ready" @click="submit">确认创建有效任务</button><button class="pa-btn" :disabled="busy" @click="preview = null">返回修改</button></div>
    </div>
  </section>
</template>

<style scoped>
.source-panel { padding: 16px; }.section-heading { margin-bottom: 14px; }.section-heading p { font-size: 11px; }
.source-search { flex: 1; min-width: 0; }.source-search .pa-input { font-size: 12px; min-height: 38px; }.source-search button { font-size: 12px; padding: 8px 11px; }.source-search button svg { display: none; }.source-check { display: flex; align-items: center; gap: 5px; font-size: 11px; white-space: nowrap; }
.source-browser { display: grid; grid-template-columns: minmax(160px, .78fr) minmax(0, 1.25fr); gap: 14px; min-height: 235px; }
.source-versions { border: 1px solid var(--border); border-radius: 9px; background: #fcfdff; overflow: hidden; display: flex; flex-direction: column; }
.source-list-heading { display: flex; align-items: center; justify-content: space-between; gap: 8px; min-height: 35px; padding: 5px 10px; font-size: 11px; color: var(--muted); border-bottom: 1px solid var(--border); }.source-list-heading > span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.source-release-list { max-height: 210px; overflow-y: auto; }.source-release { width: 100%; display: flex; gap: 9px; align-items: center; padding: 12px 10px; background: transparent; border: 0; border-bottom: 1px solid var(--border); border-left: 2px solid transparent; color: var(--text); text-align: left; }.source-release > svg { flex-shrink: 0; }.source-release > span { flex: 1; min-width: 0; }.source-release strong { display: block; font-size: 13px; overflow-wrap: anywhere; }.source-release small { display: block; font-size: 10px; color: var(--muted); margin-top: 6px; }.source-release:hover { background: var(--surface-subtle); }.source-release[aria-pressed='true'] { background: #ecf8f2; border-left-color: var(--green); }.source-release[aria-pressed='true'] > svg { color: var(--green); }
.source-no-release { padding: 12px; font-size: 12px; line-height: 1.8; color: var(--muted); }.source-pagination { display: flex; justify-content: center; align-items: center; gap: 8px; padding: 5px; margin-top: auto; border-top: 1px solid var(--border); font-size: 11px; }.source-pagination .pa-btn { font-size: 11px; min-height: 30px; padding: 4px 8px; }
.source-assets { min-width: 0; }.source-asset-heading { display: flex; align-items: flex-start; justify-content: space-between; gap: 6px; margin: 0 0 10px; }.source-asset-heading strong { font-size: 13px; overflow-wrap: anywhere; }.source-asset-heading p { font-size: 10px; color: var(--muted); margin: 5px 0 0; }.source-asset-scroll { max-height: 217px; }.source-asset-scroll th { position: sticky; top: 0; z-index: 1; }.source-asset-scroll th, .source-asset-scroll td { padding: 7px 8px; }.source-asset-scroll th:first-child { width: 40px; }.source-asset-scroll th:last-child { width: 75px; }.source-asset-scroll td:last-child { white-space: nowrap; font-size: 10px; color: var(--muted); }.asset strong { display: block; font-weight: 500; font-size: 11px; overflow-wrap: anywhere; }.asset small { display: block; font-size: 9px; line-height: 1.7; color: var(--muted); margin-top: 3px; }.asset small.pa-error { color: var(--red); }
.source-empty { min-height: 145px; padding: 20px 8px; }.source-empty p { font-size: 11px; }.source-selection { display: flex; align-items: center; justify-content: space-between; gap: 8px; margin-top: 8px; font-size: 11px; color: var(--muted); }.source-selection .pa-btn { font-size: 11px; min-height: 30px; padding: 5px 9px; }
.source-batch { display: flex; align-items: center; gap: 10px; padding: 9px; margin-top: 12px; border: 1px solid var(--border); border-radius: 9px; background: var(--surface-subtle); }.source-batch textarea { resize: vertical; min-height: 48px; max-height: 180px; font-size: 11px; }.source-batch .pa-btn { font-size: 12px; }.source-note { font-size: 10px; line-height: 1.7; color: var(--muted); margin: 9px 0 0; }
.source-feedback { margin: 10px 0; font-size: 12px; }.source-preview { border-top: 1px solid var(--border); padding-top: 14px; margin-top: 14px; }.source-preview h3 { font-size: 15px; margin: 0 0 10px; }.source-preview p, .source-preview li { font-size: 12px; line-height: 1.7; overflow-wrap: anywhere; }.source-preview ul { max-height: 190px; overflow-y: auto; padding-left: 20px; }.source-preview li { margin-bottom: 8px; }.source-preview-actions { display: flex; align-items: center; flex-wrap: wrap; gap: 8px; }.source-preview-actions label { display: flex; align-items: center; gap: 8px; font-size: 12px; }.source-preview-actions select { width: auto; }
@media (max-width: 520px) { .source-search { flex-basis: 100%; }.source-browser { grid-template-columns: minmax(0, 1fr); }.source-release-list { max-height: 160px; }.source-batch { flex-wrap: wrap; }.source-batch textarea { flex-basis: 100%; }.source-batch button { margin-left: auto; }.source-preview-actions { align-items: stretch; flex-direction: column; } }
.source-repository-column { min-width: 0; display: flex; flex-direction: column; gap: 10px; }.source-search { flex: none; }.source-versions { flex: 1; }.source-panel > .section-heading .section-heading__body { display: flex; align-items: baseline; gap: 12px; }.source-panel > .section-heading p { margin: 0; }.source-asset-heading { min-height: 39px; }.source-asset-scroll { max-height: 165px; }.source-asset-scroll td { padding: 5px 8px; }.asset small { font-size: 10px; }.source-release-list { max-height: 224px; }.source-release { padding: 10px; }.source-batch { padding: 6px; margin-top: 9px; gap: 6px; }.source-batch textarea { height: 48px; min-height: 48px; padding: 5px 8px; font-size: 11px; }.source-batch .pa-btn { padding: 6px 8px; min-height: 34px; font-size: 11px; }.source-batch .pa-btn svg { display: none; }
@media (max-width: 760px) { .source-panel > .section-heading .section-heading__body { display: block; }.source-panel > .section-heading p { margin-top: 3px; } }
.source-release { padding: 8px 10px; }.source-release small { margin-top: 3px; line-height: 1.5; font-size: 11px; }.source-release strong { line-height: 1.4; }.source-search .pa-input { font-size: 13px; }.source-asset-heading p, .source-note { font-size: 11px; }.source-asset-scroll th { font-size: 12px; }.asset strong { font-size: 12px; }
</style>
