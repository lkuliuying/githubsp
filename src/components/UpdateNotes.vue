<script setup lang="ts">
import { computed, onUnmounted, ref, watch } from 'vue'
import { downloadsApi, errorMessage } from '../services/downloads'
import { normalizeUpdateLink, renderUpdateNotes } from '../updateNotes'

const props = defineProps<{ notes: string; ready: boolean }>()
const html = computed(() => renderUpdateNotes(props.notes))
const opening = ref(false), error = ref('')
let disposed = false, generation = 0
watch(() => props.notes, () => { generation++; error.value = '' })
onUnmounted(() => { disposed = true })

async function openLink(event: MouseEvent) {
  const target = event.target instanceof Element ? event.target.closest('button[data-update-url]') : null
  if (!target || !(event.currentTarget instanceof Element) || !event.currentTarget.contains(target)) return
  event.preventDefault()
  if (!props.ready || opening.value || event.button > 1) return
  const url = normalizeUpdateLink(target.getAttribute('data-update-url') || '')
  if (!url) return
  const requestGeneration = generation
  opening.value = true; error.value = ''
  try { await downloadsApi.openUpdateLink(url) }
  catch (cause) { if (!disposed && requestGeneration === generation) error.value = errorMessage(cause) }
  finally { if (!disposed) opening.value = false }
}
</script>

<template>
  <div class="update-markdown" :class="{ 'update-markdown--disabled': !ready || opening }">
    <div class="update-markdown__content" role="region" aria-label="更新说明正文" tabindex="0"
      :aria-busy="opening" :aria-disabled="!ready || opening" @click="openLink" @auxclick="openLink" v-html="html" />
    <p v-if="opening" class="update-markdown__feedback pa-muted" role="status">正在打开链接…</p>
    <p v-if="error" class="update-markdown__feedback pa-error" role="alert">{{ error }}</p>
  </div>
</template>

<style scoped>
.update-markdown { min-width: 0; margin-top: 12px; }
.update-markdown__content { max-height: 230px; overflow: auto; padding: 2px 5px 3px 2px; color: var(--text); font-size: 12px; line-height: 1.8; overflow-wrap: anywhere; }
.update-markdown__content :deep(:first-child) { margin-top: 0; }
.update-markdown__content :deep(:last-child) { margin-bottom: 0; }
.update-markdown__content :deep(h1), .update-markdown__content :deep(h2), .update-markdown__content :deep(h3), .update-markdown__content :deep(h4), .update-markdown__content :deep(h5), .update-markdown__content :deep(h6) { margin: 16px 0 8px; font-size: 14px; line-height: 1.5; font-weight: 650; }
.update-markdown__content :deep(h1) { font-size: 17px; }
.update-markdown__content :deep(h2) { font-size: 15px; }
.update-markdown__content :deep(p) { margin: 8px 0; }
.update-markdown__content :deep(ul), .update-markdown__content :deep(ol) { margin: 8px 0; padding-left: 23px; }
.update-markdown__content :deep(li) { padding-left: 2px; }
.update-markdown__content :deep(li > p), .update-markdown__content :deep(li > ul), .update-markdown__content :deep(li > ol) { margin: 3px 0; }
.update-markdown__content :deep(button[data-update-url]) { display: inline; padding: 0; border: 0; background: transparent; color: var(--green-hover); font: inherit; line-height: inherit; text-align: left; text-decoration: underline; text-underline-offset: 2px; white-space: normal; overflow-wrap: anywhere; }
.update-markdown--disabled :deep(button[data-update-url]) { cursor: not-allowed; }
.update-markdown__content :deep(code) { border-radius: 4px; padding: 1px 4px; background: var(--green-soft); font-family: Consolas, monospace; font-size: 11px; }
.update-markdown__content :deep(pre) { max-width: 100%; overflow-x: auto; margin: 10px 0; padding: 10px; border: 1px solid var(--green-border); border-radius: 6px; background: var(--surface); white-space: pre; overflow-wrap: normal; }
.update-markdown__content :deep(pre code) { padding: 0; background: transparent; }
.update-markdown__content :deep(blockquote) { margin: 10px 0; padding: 0 10px; border-left: 3px solid var(--green-border); color: var(--muted); }
.update-markdown__content :deep(table) { display: block; max-width: 100%; overflow-x: auto; border-collapse: collapse; margin: 10px 0; }
.update-markdown__content :deep(th), .update-markdown__content :deep(td) { border: 1px solid var(--green-border); padding: 5px 9px; text-align: left; min-width: 80px; }
.update-markdown__content :deep(th) { background: var(--green-soft); }
.update-markdown__content :deep(hr) { border: 0; border-top: 1px solid var(--green-border); margin: 14px 0; }
.update-markdown__feedback { font-size: 12px; line-height: 1.8; margin: 8px 0 0; overflow-wrap: anywhere; }
</style>
