<script setup lang="ts">
import { onUnmounted, ref } from 'vue'
import { PhArrowsClockwise, PhListBullets, PhArrowSquareOut, PhInfo } from '@phosphor-icons/vue'
import type { UpdateResult } from '../types'
import { downloadsApi, errorMessage } from '../services/downloads'
import UpdateNotes from './UpdateNotes.vue'

const props = defineProps<{ ready: boolean }>()
const result = ref<UpdateResult | null>(null), busy = ref(false), opening = ref(false), error = ref('')
const openError = ref('')
const checkedAt = ref<number | null>(null)
let disposed = false
onUnmounted(() => { disposed = true })
async function check() {
  if (busy.value || !props.ready) return
  busy.value = true; error.value = ''; openError.value = ''
  try {
    const value = await downloadsApi.checkUpdate()
    if (!disposed) { result.value = value; checkedAt.value = Date.now() }
  } catch (cause) { if (!disposed) error.value = errorMessage(cause) }
  finally { if (!disposed) busy.value = false }
}
async function open() {
  if (!result.value?.url || opening.value || !props.ready) return
  opening.value = true; openError.value = ''
  try { await downloadsApi.openRelease(result.value.url) }
  catch (cause) { if (!disposed) openError.value = errorMessage(cause) }
  finally { if (!disposed) opening.value = false }
}
</script>

<template>
  <section class="pa-panel update-panel">
    <div class="section-heading"><span class="section-heading__icon"><PhArrowsClockwise :size="24" /></span><div class="section-heading__body"><h2>GitHubSP 软件更新</h2><p>检查官方发布信息，由你前往发布页获取新版。</p></div></div>
    <div class="update-grid">
      <div class="update-current"><div><strong>当前版本</strong><span class="pa-badge">v{{ result?.current || '0.2.2' }}</span><span v-if="result?.latest" class="pa-badge pa-badge--success">{{ result.latest }}</span></div><p>上次检查：{{ checkedAt ? new Date(checkedAt).toLocaleString() : '尚未检查' }}</p><p>更新方式：手动获取新版</p><button class="pa-btn update-check" :disabled="!ready || busy" @click="check"><PhArrowsClockwise :size="18" />{{ busy ? '正在检查…' : '检查软件更新' }}</button></div>
      <div class="update-notes"><div><PhListBullets :size="22" /><h3>更新说明</h3></div><p v-if="error" class="pa-error" role="alert">{{ error }}</p><div v-else-if="result"><p role="status">{{ result.message }}</p><p v-if="result.nextCheck" class="pa-muted">下次可检查：{{ new Date(result.nextCheck).toLocaleString() }}</p><UpdateNotes v-if="result.notes" :notes="result.notes" :ready="ready" /></div><p v-else class="pa-muted">点击“检查软件更新”，查看可用版本和更新说明。</p></div>
      <div class="update-release"><button class="pa-btn" :disabled="!ready || !result?.url || opening" @click="open"><PhArrowSquareOut :size="20" />打开官方发布页</button><p><PhInfo :size="15" />查看完整的版本历史与更新日志</p><p v-if="openError" class="pa-error" role="alert">{{ openError }}</p></div>
    </div>
  </section>
</template>

<style scoped>
.update-grid { display: grid; grid-template-columns: minmax(230px, .95fr) minmax(0, 1.3fr) minmax(200px, .65fr); align-items: stretch; gap: 20px; }.update-current { position: relative; padding-right: 12px; }.update-current > div { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; font-size: 12px; }.update-current > p { color: var(--muted); font-size: 11px; margin: 9px 0; }.update-check { margin-top: 7px; color: var(--green); border-color: #72cda2; min-height: 36px; font-size: 12px; }.update-notes { background: #edf9f3; border-radius: 10px; padding: 16px; min-width: 0; }.update-notes > div:first-child { display: flex; gap: 9px; align-items: center; color: #087c4d; }.update-notes h3 { font-size: 14px; margin: 0; color: var(--text); }.update-notes p { font-size: 12px; line-height: 1.8; margin: 9px 0 0; overflow-wrap: anywhere; }.update-release { display: flex; flex-direction: column; justify-content: center; align-items: center; padding: 14px; border: 1px solid var(--border); border-radius: 10px; }.update-release button { width: 100%; font-size: 12px; }.update-release p { display: flex; gap: 6px; align-items: flex-start; color: var(--muted); font-size: 10px; margin: 12px 0 0; line-height: 1.6; }.update-release p svg { flex-shrink: 0; }
@media (max-width: 1000px) { .update-grid { grid-template-columns: minmax(0, .8fr) minmax(0, 1.2fr); }.update-release { grid-column: 1 / -1; flex-direction: row; gap: 18px; justify-content: flex-start; }.update-release button { width: auto; }.update-release p { margin: 0; } }
@media (max-width: 560px) { .update-grid { grid-template-columns: minmax(0, 1fr); gap: 14px; }.update-release { flex-direction: column; align-items: stretch; }.update-release button { width: 100%; } }
.update-panel > .section-heading .section-heading__body { display: flex; align-items: baseline; gap: 16px; }.update-panel > .section-heading p { margin: 0; }.update-current { display: grid; grid-template-columns: minmax(0, 1fr) auto; gap: 5px 10px; align-content: center; }.update-current > div, .update-current > p { grid-column: 1; margin: 0; }.update-current > p { line-height: 1.7; }.update-check { grid-column: 2; grid-row: 1 / 4; align-self: center; margin: 0; }.update-grid { grid-template-columns: minmax(280px, 1fr) minmax(0, 1.25fr) minmax(200px, .65fr); }.update-notes { padding: 14px; }
@media (max-width: 1100px) { .update-grid { grid-template-columns: minmax(0, 1fr) minmax(0, 1.1fr); }.update-release { grid-column: 1 / -1; flex-direction: row; gap: 18px; justify-content: flex-start; }.update-release button { width: auto; }.update-current { display: block; }.update-current > p { margin: 8px 0; }.update-current > button { margin-top: 6px; } }
@media (max-width: 680px) { .update-panel > .section-heading .section-heading__body { display: block; }.update-panel > .section-heading p { margin-top: 4px; } }
@media (max-width: 560px) { .update-grid { grid-template-columns: minmax(0, 1fr); }.update-release { flex-direction: column; align-items: stretch; }.update-release button { width: 100%; } }
.update-release .pa-error { color: var(--red); }
</style>
