<script setup lang="ts">
import { nextTick, onMounted, onUnmounted, ref } from 'vue'
import { PhCheckCircle, PhX } from '@phosphor-icons/vue'
import { formatElapsed } from '../format'
import { noticeApi } from '../services/notices'
import { errorMessage } from '../services/downloads'
import type { DownloadNoticeView } from '../types'

const view = ref<DownloadNoticeView>({ revision: -1, notice: null })
const error = ref(''), busy = ref(false)
let disposed = false, hovering = false
let unlisten: (() => void) | undefined

async function apply(value: DownloadNoticeView) {
  if (disposed || value.revision <= view.value.revision) return
  view.value = value; error.value = ''
  if (!value.notice) { hovering = false; return }
  await nextTick()
  if (disposed || view.value.revision !== value.revision) return
  try {
    await noticeApi.hover(value.revision, hovering)
    await noticeApi.presented(value.revision)
  } catch (cause) { if (!disposed && view.value.revision === value.revision) error.value = errorMessage(cause) }
}

async function hover(value: boolean) {
  hovering = value
  if (!view.value.notice) return
  try { await noticeApi.hover(view.value.revision, value) }
  catch (cause) { if (!disposed) error.value = errorMessage(cause) }
}

async function act(action: 'open' | 'dismiss') {
  if (busy.value || !view.value.notice) return
  busy.value = true; error.value = ''
  try { await noticeApi[action](view.value.revision) }
  catch (cause) { if (!disposed) error.value = errorMessage(cause) }
  finally { if (!disposed) busy.value = false }
}

onMounted(async () => {
  try {
    const stop = await noticeApi.subscribe(value => { void apply(value) })
    if (disposed) { stop(); return }
    unlisten = stop
    await apply(await noticeApi.read())
  } catch (cause) { if (!disposed) error.value = errorMessage(cause) }
})
onUnmounted(() => { disposed = true; unlisten?.() })
</script>

<template>
  <main class="download-notice" @mouseenter="hover(true)" @mouseleave="hover(false)" @keydown.esc="act('dismiss')">
    <template v-if="view.notice">
      <div class="download-notice-heading"><PhCheckCircle :size="24" weight="fill" /><strong>{{ view.notice.count > 1 ? `已完成 ${view.notice.count} 项下载` : '下载完成' }}</strong><button class="pa-btn pa-btn--icon" aria-label="关闭提醒" title="关闭提醒" :disabled="busy" @click="act('dismiss')"><PhX :size="17" /></button></div>
      <p class="download-notice-file" :title="view.notice.filename">{{ view.notice.count > 1 ? '最近完成：' : '' }}{{ view.notice.filename }}</p>
      <div class="download-notice-footer"><span>{{ view.notice.count > 1 ? '该项耗时' : '总耗时' }} {{ formatElapsed(view.notice.elapsedMs) }}<small v-if="view.notice.elapsedIsPartial">（记录不完整）</small></span><button class="pa-btn pa-btn--primary" :disabled="busy" @click="act('open')">查看下载</button></div>
    </template>
    <p v-if="error" class="pa-error" role="alert">{{ error }}</p>
  </main>
</template>

<style scoped>
.download-notice { min-height: 100vh; padding: 14px 16px; background: var(--surface); border: 1px solid var(--border-strong); overflow-wrap: anywhere; }
.download-notice-heading { display: flex; align-items: center; gap: 8px; color: var(--green); }
.download-notice-heading strong { flex: 1; font-size: 14px; }
.download-notice-heading > svg { flex-shrink: 0; }
.download-notice-heading .pa-btn { min-height: 28px; width: 28px; padding: 4px; }
.download-notice-file { margin: 10px 0; font-size: 12px; line-height: 1.5; display: -webkit-box; -webkit-line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden; }
.download-notice-footer { display: flex; align-items: center; justify-content: space-between; gap: 10px; }
.download-notice-footer > span { font-size: 11px; color: var(--muted); font-variant-numeric: tabular-nums; }
.download-notice-footer small { display: block; }
.download-notice-footer .pa-btn { flex-shrink: 0; min-height: 30px; padding: 6px 10px; font-size: 12px; }
.download-notice .pa-error { margin: 6px 0 0; font-size: 11px; }
</style>
