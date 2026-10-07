<script setup lang="ts">
import { computed, ref } from 'vue'
import { builtinRoutes, type DownloadTask, type Snapshot } from '../types'
import { downloadsApi, errorMessage } from '../services/downloads'
const props = defineProps<{ task: DownloadTask; busy: boolean }>()
const emit = defineEmits<{ snapshot: [value: Snapshot]; pause: [id: string]; redownload: [task: DownloadTask]; favorite: [repository: string] }>()
const working = ref(false), error = ref(''), selected = ref(''), dialog = ref<HTMLDialogElement>()
const active = computed(() => ['probing', 'downloading', 'retrying', 'verifying', 'pausing', 'cancelling'].includes(props.task.status))
async function change(event: Event) {
  selected.value = (event.target as HTMLSelectElement).value
  ;(event.target as HTMLSelectElement).value = props.task.preferredRoute || ''
  if (props.task.downloaded > 0) dialog.value?.showModal()
  else await save(false)
}
async function save(restart: boolean) {
  dialog.value?.close(); working.value = true; error.value = ''
  try { emit('snapshot', await downloadsApi.changeRoute(props.task.id, selected.value || null, restart)) }
  catch (cause) { error.value = errorMessage(cause) }
  finally { working.value = false }
}
</script>
<template>
  <div class="route-controls">
    <p v-if="task.failure" class="error">{{ task.failure.action }}</p>
    <div class="controls"><button v-if="active" class="pa-btn" :disabled="busy || working" @click="emit('pause', task.id)">先暂停再更改线路</button><label v-else-if="['paused', 'failed', 'queued'].includes(task.status)">下载线路 <select :key="task.revision" class="pa-input" :value="task.preferredRoute || ''" :disabled="busy || working" @change="change"><option v-for="route in builtinRoutes" :key="route.id" :value="route.id">{{ route.name }}</option></select></label><button class="pa-btn" :disabled="busy || working" @click="emit('redownload', task)">另选目录重新下载</button><button v-if="task.repository" class="pa-btn" :disabled="busy || working" @click="emit('favorite', task.repository)">收藏项目</button></div>
    <p v-if="error" class="error" role="alert">{{ error }}</p>
    <dialog ref="dialog"><h3>更改下载线路？</h3><p>需要换线时会丢弃原线路分片并从头下载。自动模式会先尝试安全复用原线路进度。</p><div class="controls"><button class="pa-btn" autofocus @click="dialog?.close()">保留原线路</button><button class="pa-btn pa-btn--primary" @click="save(true)">确认更改</button></div></dialog>
  </div>
</template>
<style scoped>
.route-controls { margin: 0; } .controls { display: flex; flex-wrap: wrap; gap: 8px; align-items: center; } label { display: flex; align-items: center; gap: 8px; } label, p { font-size: 12px; line-height: 1.7; } .error { color: var(--red); } select { max-width: 170px; } dialog { max-width: min(420px, calc(100vw - 48px)); padding: 24px; border: 1px solid var(--border); border-radius: 12px; color: var(--text); } dialog::backdrop { background: #17274766; }
</style>
