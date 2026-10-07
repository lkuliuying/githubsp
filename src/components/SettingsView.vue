<script setup lang="ts">
import { computed, onUnmounted, ref, watch } from 'vue'
import { PhGearSix, PhDownloadSimple, PhMonitor, PhStar, PhInfo, PhArrowCounterClockwise, PhFloppyDisk, PhCheckCircle } from '@phosphor-icons/vue'
import { defaultSettings, type Settings, type Snapshot } from '../types'
import { downloadsApi, errorMessage } from '../services/downloads'

const props = defineProps<{ settings: Settings; ready: boolean }>()
const emit = defineEmits<{ snapshot: [value: Snapshot] }>()
const form = ref({ ...props.settings }), busy = ref(false), error = ref(''), saved = ref(false), resetMessage = ref('')
const lastLimited = ref(props.settings.limitKib > 0 ? props.settings.limitKib : 1024)
const unlimited = computed(() => form.value.limitKib === 0)
const dirty = computed(() => JSON.stringify(form.value) !== JSON.stringify(props.settings))
let disposed = false
onUnmounted(() => { disposed = true })
watch(() => props.settings, (value, previous) => {
  if (!busy.value && JSON.stringify(value) !== JSON.stringify(previous)) form.value = { ...value }
}, { deep: true })
watch(form, () => { saved.value = false; resetMessage.value = '' }, { deep: true, flush: 'sync' })
function toggleLimit() {
  if (unlimited.value) form.value.limitKib = lastLimited.value
  else {
    if (Number.isInteger(form.value.limitKib) && form.value.limitKib > 0 && form.value.limitKib <= 10000000) lastLimited.value = form.value.limitKib
    form.value.limitKib = 0
  }
}
function restoreDefaults() { form.value = defaultSettings(); error.value = ''; resetMessage.value = '已恢复默认值，保存设置后生效。' }
async function save() {
  if (busy.value || !props.ready) return
  if (!Number.isInteger(form.value.limitKib) || form.value.limitKib < 0 || form.value.limitKib > 10000000) { error.value = '请输入 0 至 10000000 KiB/s 的整数'; return }
  busy.value = true; error.value = ''; saved.value = false; resetMessage.value = ''
  try { const value = await downloadsApi.saveSettings({ ...form.value }); if (!disposed) { emit('snapshot', value); saved.value = true } }
  catch (cause) { if (!disposed) error.value = errorMessage(cause) }
  finally { if (!disposed) busy.value = false }
}
</script>

<template>
  <div class="settings-view">
    <section class="pa-panel"><div class="section-heading section-heading--page"><span class="section-heading__icon"><PhGearSix :size="30" weight="fill" /></span><div class="section-heading__body"><h1>应用设置</h1><p>管理 GitHubSP 的常用设置，让下载更符合你的使用习惯。</p></div></div></section>
    <form id="settings-form" class="settings-grid" @submit.prevent="save">
      <section class="pa-panel settings-speed">
        <div class="section-heading"><span class="section-heading__icon"><PhDownloadSimple :size="24" weight="bold" /></span><div class="section-heading__body"><h2>下载限速</h2><p>设置全局下载速度限制，按需分配网络带宽。</p></div></div>
        <div class="settings-inset">
          <div class="settings-option"><div><h3 id="unlimited-label">不限速</h3><p>开启后按当前可用速度下载。</p></div><button type="button" class="pa-switch" role="switch" :aria-checked="unlimited" aria-labelledby="unlimited-label" :disabled="!ready || busy" @click="toggleLimit" /></div>
          <div class="settings-limit"><label for="rate-limit">下载速度限制</label><p>输入 0 表示不限速，保存后立即生效。</p><div class="settings-limit-value"><input id="rate-limit" v-model.number="form.limitKib" class="pa-input" type="number" min="0" max="10000000" step="1" :disabled="!ready || busy" aria-describedby="rate-description" /><span>KiB/s</span></div>
            <label class="pa-sr-only" for="rate-range">快捷调整限速，0 至 102400 KiB/s</label><input id="rate-range" class="settings-range" type="range" min="0" max="102400" step="256" :value="Math.min(102400, Math.max(0, form.limitKib))" :disabled="!ready || busy" @input="form.limitKib = Number(($event.target as HTMLInputElement).value)" /><div class="settings-range-labels"><span>不限速</span><span>100 MiB/s</span></div>
            <p v-if="form.limitKib > 102400" class="pa-muted">当前限速超过滑块的 100 MiB/s 范围，请使用上方输入框调整。</p>
          </div>
          <p id="rate-description" class="pa-message"><PhInfo :size="19" weight="fill" /><span>仅限制 GitHubSP 的下载任务，两个分片共享额度。线路检测流量不受限速影响。</span></p>
        </div>
      </section>
      <div class="settings-side">
        <section class="pa-panel"><div class="section-heading"><span class="section-heading__icon"><PhMonitor :size="23" /></span><div class="section-heading__body"><h2>窗口行为</h2><p>设置关闭应用窗口时的行为。</p></div></div>
          <div class="settings-inset"><label class="settings-option"><span><strong>关闭窗口时收起到托盘</strong><small>保持应用运行，下载任务可在后台继续。</small></span><input v-model="form.closeToTray" class="pa-switch" type="checkbox" :disabled="!ready || busy" /></label><p class="pa-message"><PhInfo :size="19" weight="fill" /><span>默认关闭窗口会保存进度并退出。托盘菜单的“保存进度并退出”始终退出应用。</span></p></div>
        </section>
        <section class="pa-panel"><div class="section-heading"><span class="section-heading__icon"><PhStar :size="24" weight="fill" /></span><div class="section-heading__body"><h2>收藏更新检查</h2><p>在应用运行期间，定期检查收藏项目的新版本。</p></div></div>
          <div class="settings-inset"><label class="settings-option"><span><strong>自动检查收藏更新</strong><small>发现新正式版时在应用内生成提醒。</small></span><input v-model="form.autoCheck" class="pa-switch" type="checkbox" :disabled="!ready || busy" /></label><div class="settings-interval"><strong>检查间隔</strong><span class="pa-badge">每 6 小时</span></div><p class="pa-message"><PhInfo :size="19" weight="fill" /><span>退出应用后停止检查，不创建后台服务或开机自启。首次成功检查建立版本基线。</span></p></div>
        </section>
      </div>
    </form>
    <slot />
    <div class="settings-footer"><div class="settings-feedback"><p v-if="error" role="alert" class="pa-error">{{ error }}</p><p v-else-if="saved" role="status" class="settings-saved"><PhCheckCircle :size="18" weight="fill" />设置已保存</p><p v-else-if="resetMessage" role="status">{{ resetMessage }}</p><p v-else-if="dirty">有尚未保存的设置</p></div><button type="button" class="pa-btn" :disabled="!ready || busy" @click="restoreDefaults"><PhArrowCounterClockwise :size="18" />恢复默认</button><button form="settings-form" class="pa-btn pa-btn--primary" :disabled="!ready || busy"><PhFloppyDisk :size="18" />{{ busy ? '保存中…' : '保存设置' }}</button></div>
  </div>
</template>

<style scoped>
.settings-view { display: flex; flex-direction: column; gap: 16px; }.settings-grid { display: grid; grid-template-columns: minmax(0, 1fr) minmax(0, 1fr); gap: 16px; }.settings-side { display: flex; flex-direction: column; gap: 16px; }.settings-inset { border: 1px solid var(--border); border-radius: 10px; padding: 0 14px 14px; }.settings-option { display: flex; align-items: center; gap: 20px; justify-content: space-between; padding: 16px 0; }.settings-option h3, .settings-option strong { margin: 0; font-size: 13px; font-weight: 600; }.settings-option p, .settings-option small { margin: 6px 0 0; display: block; font-size: 12px; color: var(--muted); line-height: 1.7; }.settings-option .pa-switch { border: 0; flex-shrink: 0; }.settings-inset > .pa-message { margin: 0; font-size: 11px; }.settings-limit { border-top: 1px solid var(--border); padding-top: 16px; margin-bottom: 28px; }.settings-limit > label { font-size: 13px; font-weight: 600; }.settings-limit > p { font-size: 12px; color: var(--muted); margin: 7px 0 18px; }.settings-limit-value { display: flex; align-items: center; gap: 12px; padding: 14px; border-radius: 9px; background: var(--surface-subtle); font-size: 13px; }.settings-limit-value input { max-width: 180px; background: var(--surface); font-weight: 600; }.settings-range { display: block; width: 100%; margin: 24px 0 12px; accent-color: var(--green); height: 6px; cursor: pointer; }.settings-range-labels { display: flex; justify-content: space-between; font-size: 11px; color: var(--muted); }.settings-presets { display: flex; gap: 8px; margin-top: 20px; }.settings-presets button { font-size: 11px; min-height: 33px; }.settings-interval { display: flex; justify-content: space-between; align-items: center; padding: 12px 0; border-top: 1px solid var(--border); font-size: 12px; }.settings-interval strong { font-weight: 500; }.settings-interval .pa-badge { padding: 6px 14px; }
.settings-footer { display: flex; justify-content: flex-end; align-items: center; gap: 12px; }.settings-footer > button { min-width: 140px; }.settings-feedback { margin-right: auto; min-width: 0; font-size: 12px; color: var(--muted); }.settings-feedback p { margin: 0; line-height: 1.7; }.settings-saved { color: var(--green); display: flex; gap: 7px; align-items: center; }
@media (max-width: 850px) { .settings-grid { grid-template-columns: minmax(0, 1fr); }.settings-side { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); }.settings-side .section-heading { align-items: flex-start; }.settings-side .settings-option { align-items: flex-start; } }
@media (max-width: 680px) { .settings-side { grid-template-columns: minmax(0, 1fr); }.settings-footer { flex-wrap: wrap; }.settings-feedback { flex-basis: 100%; }.settings-footer > button { min-width: 0; flex: 1; } }
.settings-grid .pa-panel { padding: 16px; }.settings-grid .section-heading { margin-bottom: 12px; }.settings-grid .settings-option { padding: 12px 0; }.settings-grid .settings-inset > .pa-message { padding: 9px 11px; }.settings-limit { margin-bottom: 20px; }.settings-range { margin-top: 18px; }.settings-interval { padding: 10px 0; }
</style>
