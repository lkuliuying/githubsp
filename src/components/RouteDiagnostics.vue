<script setup lang="ts">
import { computed } from 'vue'
import { PhBroadcast, PhArrowClockwise, PhCheckCircle, PhInfo, PhCircle } from '@phosphor-icons/vue'
import { builtinRoutes, type DiagnosticContext, type RouteReport } from '../types'
import { formatBytes } from '../format'

const props = defineProps<{ reports: RouteReport[]; context?: DiagnosticContext | null; diagnosing: boolean; ready: boolean; hasActive: boolean; url: string }>()
defineEmits<{ diagnose: [] }>()
const rows = computed(() => builtinRoutes.filter(route => route.id).map(route => ({
  ...route, report: props.diagnosing ? undefined : props.reports.find(report => report.id === route.id),
})))
const best = computed(() => props.diagnosing ? undefined : [...props.reports].filter(report => report.available && report.checkedAt).sort((a, b) => b.bytesPerSecond - a.bytesPerSecond)[0])
const hasChecked = computed(() => props.reports.some(report => report.checkedAt > 0))
const canDiagnose = computed(() => props.ready && !props.diagnosing)
const disabledReason = computed(() => !props.ready ? '任务管理器尚未就绪，暂时无法检测。' : props.diagnosing ? '正在检测线路，请等待本轮检测结束。' : '')
const sourceLabels = { default: '默认测试文件', input: '指定附件', download: '下载自动检测' }
</script>

<template>
  <section class="pa-panel route-panel" aria-labelledby="routes-title">
    <div class="section-heading">
      <span class="section-heading__icon"><PhBroadcast :size="23" /></span>
      <div class="section-heading__body"><h2 id="routes-title">线路检测</h2><p>检测线路可用性与速度，结果仅供参考。</p></div>
      <button class="pa-btn route-refresh" :disabled="!canDiagnose" :title="disabledReason || '手动检测线路'" aria-describedby="routes-status" @click="$emit('diagnose')"><PhArrowClockwise :size="17" :class="{ 'route-spin': diagnosing }" />{{ diagnosing ? '检测中' : '检测线路' }}</button>
    </div>
    <p v-if="context" class="route-target"><span>{{ sourceLabels[context.source] }}：</span><strong :title="context.filename">{{ context.filename }}</strong></p>
    <p v-else-if="hasChecked" class="route-target">历史检测结果</p>
    <div class="pa-table-scroll">
      <table class="pa-table"><thead><tr><th scope="col">线路</th><th scope="col">下载速度</th><th scope="col">状态</th></tr></thead>
        <tbody><tr v-for="row in rows" :key="row.id">
          <td><strong>{{ row.name }}</strong><small v-if="row.report?.checkedAt">{{ new Date(row.report.checkedAt).toLocaleTimeString() }}</small></td>
          <td>{{ row.report?.checkedAt && row.report.available ? formatBytes(row.report.bytesPerSecond) + '/s' : '—' }}</td>
          <td><span class="route-state" :class="{ 'route-state--ok': row.report?.checkedAt && row.report.available, 'route-state--error': row.report?.checkedAt && !row.report.available }"><PhCircle :size="8" weight="fill" />{{ diagnosing ? '检测中' : !row.report?.checkedAt ? '尚未检测' : row.report.available ? '可用' : '不可用' }}</span><small v-if="row.report?.error" class="pa-error">{{ row.report.error }}</small></td>
        </tr></tbody>
      </table>
    </div>
    <div v-if="best" class="pa-message pa-message--success route-result"><PhCheckCircle :size="21" weight="fill" /><div><strong>最近检测最快：{{ best.name }}</strong><br />检测不会切换正在下载的线路，实际下载仍针对目标文件选线。</div></div>
    <div v-else-if="!diagnosing && hasChecked" class="pa-message pa-message--error route-result" role="status"><PhInfo :size="20" weight="fill" /><div>本轮线路检测失败，请稍后重试，或填写其他附件直链后重新检测。</div></div>
    <div v-else class="pa-message route-result"><PhInfo :size="20" weight="fill" /><div>{{ diagnosing ? '正在依次检测各条线路，请稍候。' : url.trim() ? '点击检测上方填写的附件直链。' : '无需填写链接，点击即可使用默认测试文件检测。' }}<br />检测不保存文件，也不会切换正在下载的线路。</div></div>
    <p id="routes-status" class="route-note" role="status">{{ disabledReason }}{{ hasActive ? '下载继续进行；手动检测可能短暂影响下载速度。' : '' }}{{ `每条线路最多检测约 ${formatBytes(512 * 1024)}、10 秒，检测流量不受下载限速影响。` }}</p>
  </section>
</template>

<style scoped>
.route-panel { display: flex; flex-direction: column; padding: 16px; }
.route-target { margin: 0 0 12px; font-size: 11px; line-height: 1.7; color: var(--muted); overflow-wrap: anywhere; }.route-target strong { font-weight: 500; color: var(--text); }
.section-heading { gap: 10px; }.section-heading h2 { font-size: 18px; }.section-heading p { font-size: 11px; }
.route-refresh { padding: 7px 10px; min-height: 34px; font-size: 11px; align-self: flex-start; }
.pa-table { font-size: 12px; }.pa-table td { padding: 12px; }.pa-table th { padding: 10px 12px; }
td strong { font-weight: 500; }td small { display: block; margin-top: 5px; font-size: 10px; color: var(--muted); overflow-wrap: anywhere; }td small.pa-error { color: var(--red); max-width: 140px; }
.route-state { display: inline-flex; align-items: center; gap: 5px; color: var(--muted); white-space: nowrap; }.route-state--ok { color: var(--green); }.route-state--error { color: var(--red); }
.route-result { margin-top: 16px; }.route-note { margin: auto 0 0; padding-top: 14px; color: var(--muted); font-size: 11px; line-height: 1.7; }
.route-spin { animation: route-rotate 1s linear infinite; }@keyframes route-rotate { to { transform: rotate(360deg); } }
@media (prefers-reduced-motion: reduce) { .route-spin { animation: none; } }
@media (max-width: 480px) { .section-heading { flex-wrap: wrap; }.route-refresh { margin-left: 48px; }.pa-table td { padding: 13px 8px; } }
</style>
