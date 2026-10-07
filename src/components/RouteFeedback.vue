<script setup lang="ts">
import { computed, onUnmounted, ref } from 'vue'
import type { DownloadTask, Snapshot } from '../types'
import { downloadsApi, errorMessage } from '../services/downloads'
import { formatBytes, formatElapsed, formatEta } from '../format'

const props = defineProps<{ task: DownloadTask; busy: boolean }>()
const emit = defineEmits<{ snapshot: [snapshot: Snapshot]; retry: [] }>()
const working = ref(false), error = ref(''), dialog = ref<HTMLDialogElement>()
const confirmedId = ref<string | null>(null)
const seconds = (ms: number) => Math.max(0, Math.ceil(ms / 1000))
const info = computed(() => props.task.recoveryInfo)
const suggestion = computed(() => props.task.routeSuggestion)
const confirmationValid = computed(() => suggestion.value?.id === confirmedId.value && props.task.status === 'downloading')
let disposed = false
onUnmounted(() => { disposed = true; dialog.value?.close() })

function confirm() {
  if (working.value || props.busy || !suggestion.value) return
  confirmedId.value = suggestion.value.id
  error.value = ''
  dialog.value?.showModal()
}

async function decide(apply: boolean) {
  const id = apply ? confirmedId.value : suggestion.value?.id
  if (!id || working.value || props.busy || apply && !confirmationValid.value) return
  working.value = true
  error.value = ''
  try {
    const result = await (apply ? downloadsApi.applySuggestion(props.task.id, id) : downloadsApi.dismissSuggestion(props.task.id, id))
    if (!disposed) { emit('snapshot', result); dialog.value?.close() }
  } catch (cause) {
    if (!disposed) { error.value = errorMessage(cause); dialog.value?.close() }
  } finally { if (!disposed) working.value = false }
}
</script>

<template>
  <div class="route-feedback">
    <div v-if="task.status === 'waiting_network'" class="route-feedback__line" role="status">
      <div><strong>线路暂时不可用，已保留当前进度。</strong><p>{{ info?.waitingForSlot ? '正在下载其他任务，恢复计时已暂停。' : info?.retryInMs ? `${seconds(info.retryInMs)} 秒后重新检测线路。` : '等待下载位置空闲后重新检测。' }}<span v-if="info">剩余自动恢复额度 {{ formatElapsed(info.remainingMs) }}。</span></p></div>
      <button class="pa-btn" :disabled="busy" title="跳过本次退避等待；服务端要求的冷却时间仍需遵守" @click="emit('retry')">立即重试</button>
    </div>
    <p v-else-if="task.retryInfo" class="route-feedback__retry" role="status">{{ task.retryInfo.reason }}<template v-if="task.retryInfo.phase === 'retrying'">，{{ seconds(task.retryInfo.retryInMs) }} 秒后重试（第 {{ task.retryInfo.attempt }}/{{ task.retryInfo.maxAttempts }} 次）</template></p>
    <div v-if="suggestion" class="route-feedback__line route-feedback__suggestion">
      <div><strong>持续低速，可尝试 {{ suggestion.routeName }}</strong><p>当前线路预计还需 {{ formatEta(suggestion.currentSeconds) }}；换线从头下载预计需 {{ formatEta(suggestion.suggestedSeconds) }}。<br />将重新下载已完成的 {{ formatBytes(task.downloaded) }}。测速仅供估计，当前下载继续进行。</p></div>
      <div class="route-feedback__actions"><button class="pa-btn pa-btn--primary" :disabled="busy || working" @click="confirm">换线重新下载</button><button class="pa-btn" :disabled="busy || working" @click="decide(false)">继续当前线路</button></div>
    </div>
    <p v-if="error" class="pa-error" role="alert">{{ error }}</p>
    <dialog ref="dialog" :aria-labelledby="`route-confirm-${task.id}`" @cancel="working && $event.preventDefault()">
      <h3 :id="`route-confirm-${task.id}`">确认换线并重新下载？</h3>
      <p v-if="confirmationValid">将先复核 {{ suggestion?.routeName }} 的可用性和收益，再停止当前写入。确认切换后会丢弃 {{ formatBytes(task.downloaded) }} 的已有分片并从头下载，任务仍保持自动选线。</p>
      <p v-else>下载状态或建议已变化，本次建议已失效。当前任务按最新状态继续处理。</p>
      <p v-if="working" role="status">正在复核线路，请稍候。复核失败时保留原线路下载。</p>
      <div class="route-feedback__actions"><button class="pa-btn" :disabled="working" autofocus @click="dialog?.close()">保留当前线路</button><button class="pa-btn pa-btn--primary" :disabled="busy || working || !confirmationValid" @click="decide(true)">{{ working ? '正在复核…' : '确认重新下载' }}</button></div>
    </dialog>
  </div>
</template>

<style scoped>
.route-feedback { position: sticky; left: 12px; width: 100%; max-width: calc(100cqw - 24px); padding: 10px 16px; background: #f0f6fb; border-radius: 8px; font-size: 12px; line-height: 1.7; overflow-wrap: anywhere; }
.route-feedback__line { display: flex; align-items: center; justify-content: space-between; flex-wrap: wrap; gap: 10px 18px; }.route-feedback__line > div:first-child { flex: 1 1 280px; }
.route-feedback p { margin: 4px 0 0; }.route-feedback__retry { color: var(--muted); }.route-feedback__actions { display: flex; gap: 8px; flex-wrap: wrap; }.route-feedback__suggestion { color: var(--text); }
dialog { width: min(460px, calc(100vw - 48px)); max-height: calc(100vh - 48px); overflow: auto; padding: 24px; border: 1px solid var(--border); border-radius: 12px; color: var(--text); }dialog::backdrop { background: #17274766; }dialog .route-feedback__actions { margin-top: 18px; }
@media (max-width: 480px) { .route-feedback { padding: 10px; }.route-feedback__actions { width: 100%; }.route-feedback__actions .pa-btn { flex: 1; } }
</style>
