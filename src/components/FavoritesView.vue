<script setup lang="ts">
import { computed, onUnmounted, ref } from 'vue'
import { PhStar, PhLink, PhPlus, PhCube, PhCheckCircle, PhWarning, PhClock, PhPlay, PhFolderOpen, PhTrash, PhTag, PhSquaresFour, PhListBullets, PhSortDescending, PhInfo } from '@phosphor-icons/vue'
import type { Favorite, Snapshot } from '../types'
import { downloadsApi, errorMessage } from '../services/downloads'

const props = defineProps<{ favorites: Favorite[]; ready: boolean }>()
const emit = defineEmits<{ snapshot: [value: Snapshot]; browse: [repository: string] }>()
const input = ref(''), busy = ref(false), error = ref('')
const filter = ref<'all' | 'success' | 'error' | 'pending'>('all')
const layout = ref<'cards' | 'list'>('cards'), sort = ref('checked')
const successful = computed(() => props.favorites.filter(item => item.lastSuccess && !item.error).length)
const failed = computed(() => props.favorites.filter(item => item.error).length)
const unchecked = computed(() => props.favorites.filter(item => !item.lastSuccess && !item.error).length)
const lastChecked = computed(() => Math.max(0, ...props.favorites.map(item => item.lastChecked ?? 0)))
const filters = computed(() => [{ id: 'all', label: '全部', count: props.favorites.length }, { id: 'success', label: '检查成功', count: successful.value }, { id: 'error', label: '检查失败', count: failed.value }, { id: 'pending', label: '待检查', count: unchecked.value }] as const)
const visible = computed(() => props.favorites.filter(item => filter.value === 'all' || (filter.value === 'error' ? !!item.error : filter.value === 'success' ? !!item.lastSuccess && !item.error : !item.lastSuccess && !item.error)).sort((a, b) => sort.value === 'name' ? a.repository.localeCompare(b.repository) : (b.lastChecked ?? 0) - (a.lastChecked ?? 0)))
let disposed = false
onUnmounted(() => { disposed = true })
async function run(work: () => Promise<Snapshot>) {
  if (busy.value || !props.ready) return false
  busy.value = true; error.value = ''
  try { const value = await work(); if (!disposed) emit('snapshot', value); return true }
  catch (cause) { if (!disposed) error.value = errorMessage(cause); return false }
  finally { if (!disposed) busy.value = false }
}
async function add() { const original = input.value; if (await run(() => downloadsApi.favorite(original)) && !disposed && input.value === original) input.value = '' }
async function check(repository: string | null) { await run(() => downloadsApi.checkFavorites(repository)) }
async function remove(repository: string) { await run(() => downloadsApi.removeFavorite(repository)) }
const date = (value: number | null) => value ? new Date(value).toLocaleString() : '尚未检查'
const color = (repository: string) => ['blue', 'purple', 'green', 'orange', 'pink'][[...repository].reduce((sum, char) => sum + char.charCodeAt(0), 0) % 5]
</script>

<template>
  <div class="favorites-view">
    <section class="pa-panel favorites-intro">
      <div class="section-heading section-heading--page"><span class="section-heading__icon"><PhStar :size="29" weight="fill" /></span><div class="section-heading__body"><h1>项目收藏</h1><p>收藏并关注常用的 GitHub 项目，查看最新正式版本和下载附件。</p></div></div>
      <form class="favorites-add" @submit.prevent="add"><div class="pa-input-group"><PhLink :size="22" /><label class="pa-sr-only" for="favorite-source">GitHub 仓库或版本链接</label><input id="favorite-source" v-model="input" class="pa-input" type="url" required :disabled="!ready || busy" placeholder="请输入 GitHub 仓库链接，例如 https://github.com/microsoft/PowerToys" /></div><button class="pa-btn pa-btn--primary" :disabled="!ready || busy || !input.trim()"><PhPlus :size="22" weight="bold" />添加收藏</button></form>
      <p v-if="error" class="pa-message pa-message--error" role="alert">{{ error }}</p>
    </section>
    <section class="pa-panel favorites-summary" aria-label="收藏项目概览">
      <div class="favorites-stat"><PhCube :size="32" /><div><strong>{{ favorites.length }}</strong><span>收藏项目数</span><small>最多可收藏 200 个项目</small></div></div>
      <div class="favorites-stat favorites-stat--success"><PhCheckCircle :size="32" /><div><strong>{{ successful }}</strong><span>检查成功</span><small>已获取正式版本信息</small></div></div>
      <div class="favorites-stat favorites-stat--error"><PhWarning :size="32" /><div><strong>{{ failed }}</strong><span>检查异常</span><small>{{ failed ? '保留上次成功检查结果' : '暂无检查异常' }}</small></div></div>
      <div class="favorites-check-all"><button class="pa-btn pa-btn--primary" :disabled="!ready || busy || !favorites.length" @click="check(null)"><PhPlay :size="18" weight="fill" />{{ busy ? '正在检查…' : '检查全部' }}</button><small><PhClock :size="15" />上次检查：{{ date(lastChecked || null) }}</small></div>
    </section>
    <section class="pa-panel favorites-library" :aria-busy="busy">
      <div class="favorites-toolbar"><div class="favorites-filters" aria-label="收藏状态筛选"><button v-for="item in filters" :key="item.id" class="pa-btn" :class="{ 'pa-btn--primary': filter === item.id }" :aria-pressed="filter === item.id" @click="filter = item.id">{{ item.label }}<span>{{ item.count }}</span></button></div><div class="favorites-display"><label class="favorites-sort"><PhSortDescending :size="17" /><span class="pa-sr-only">收藏排序</span><select v-model="sort" class="pa-input"><option value="checked">检查时间</option><option value="name">项目名称</option></select></label><div class="favorites-layout" aria-label="收藏显示方式"><button class="pa-btn" :class="{ 'pa-btn--soft': layout === 'cards' }" :aria-pressed="layout === 'cards'" @click="layout = 'cards'"><PhSquaresFour :size="17" />卡片</button><button class="pa-btn" :class="{ 'pa-btn--soft': layout === 'list' }" :aria-pressed="layout === 'list'" @click="layout = 'list'"><PhListBullets :size="18" />列表</button></div></div></div>
      <p v-if="busy" role="status" class="favorites-working">正在检查官方版本信息…</p>
      <div v-if="!visible.length" class="pa-empty"><PhStar :size="44" weight="light" /><h3>{{ favorites.length ? '没有符合条件的收藏' : '还没有收藏项目' }}</h3><p>{{ favorites.length ? '切换筛选条件查看其他项目。' : '从上方仓库链接或下载来源添加，常用项目触手可及。' }}</p></div>
      <div v-else class="favorites-grid" :class="{ 'favorites-grid--list': layout === 'list' }">
        <article v-for="favorite in visible" :key="favorite.id" class="favorite-card">
          <div class="favorite-heading"><span class="favorite-icon" :class="'favorite-icon--' + color(favorite.repository)"><PhCube :size="29" /></span><div class="favorite-name"><h2>{{ favorite.repository }}</h2><p>GitHub 公开仓库 · Release 版本追踪</p></div><span class="pa-badge" :class="favorite.error ? 'pa-badge--error' : favorite.lastSuccess ? 'pa-badge--success' : ''">{{ favorite.error ? '检查失败' : favorite.lastSuccess ? '检查成功' : '待检查' }}</span></div>
          <div class="favorite-version"><PhTag :size="17" /><span>最新正式版：<strong>{{ favorite.latest?.tag || '未知' }}</strong></span></div>
          <p class="favorite-time"><PhClock :size="16" />上次检查：{{ date(favorite.lastChecked) }}</p>
          <p v-if="favorite.error" class="pa-message pa-message--error favorite-error"><PhWarning :size="17" /><span>{{ favorite.error }}<small>下次可检查：{{ date(favorite.nextCheck) }} · 上次成功：{{ date(favorite.lastSuccess) }}</small></span></p>
          <p v-else-if="!favorite.lastSuccess" class="pa-message favorite-info"><PhInfo :size="17" />尚未成功检查正式版本</p>
          <div class="favorite-actions"><button class="pa-btn pa-btn--soft" :disabled="!ready || busy" @click="check(favorite.repository)"><PhPlay :size="15" weight="fill" />检查更新</button><button class="pa-btn" :disabled="!ready || busy" @click="emit('browse', favorite.repository)"><PhFolderOpen :size="17" />查看版本与附件</button><button class="pa-btn pa-btn--danger favorite-remove" :disabled="!ready || busy" title="移除收藏，不影响下载文件" @click="remove(favorite.repository)"><PhTrash :size="17" /><span class="pa-sr-only">移除收藏</span></button></div>
        </article>
      </div>
      <p class="favorites-note">首次成功检查建立正式版基线，之后有新版本时生成提醒。自动检查可在设置中启用。</p>
    </section>
  </div>
</template>

<style scoped>
.favorites-view { display: flex; flex-direction: column; gap: 16px; }.favorites-add { display: flex; gap: 18px; margin-top: 20px; }.favorites-add .pa-input-group { flex: 1; }.favorites-add input { min-height: 46px; }.favorites-add > button { min-width: 215px; }
.favorites-summary { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)) 250px; padding: 20px; gap: 24px; }.favorites-stat { display: flex; align-items: flex-start; gap: 17px; border-right: 1px solid var(--border); }.favorites-stat > svg { color: var(--green); margin-top: 6px; flex-shrink: 0; }.favorites-stat strong { display: block; font-size: 30px; line-height: 1.1; font-weight: 650; }.favorites-stat span { display: block; font-size: 13px; margin-top: 5px; }.favorites-stat small { display: block; font-size: 11px; color: var(--muted); line-height: 1.6; margin-top: 4px; }.favorites-stat--success strong { color: var(--green); }.favorites-stat--error > svg, .favorites-stat--error strong { color: var(--red); }.favorites-check-all { display: flex; flex-direction: column; gap: 10px; }.favorites-check-all button { min-height: 43px; }.favorites-check-all small { display: flex; gap: 6px; align-items: center; font-size: 11px; color: var(--muted); line-height: 1.5; }
.favorites-library { padding: 16px; }.favorites-toolbar { display: flex; justify-content: space-between; align-items: center; gap: 12px; flex-wrap: wrap; margin-bottom: 14px; }.favorites-filters { display: flex; flex-wrap: wrap; gap: 8px; }.favorites-filters button { font-size: 12px; min-height: 36px; }.favorites-filters span { padding: 0 5px; border-radius: 8px; background: #eef3f8; color: #607899; font-size: 10px; }.favorites-filters .pa-btn--primary span { background: #ffffffd9; color: var(--green); }.favorites-display { display: flex; gap: 10px; }.favorites-sort { display: flex; align-items: center; gap: 6px; }.favorites-sort select { min-height: 36px; font-size: 12px; padding: 7px; }.favorites-layout { display: flex; gap: 3px; border-left: 1px solid var(--border); padding-left: 10px; }.favorites-layout .pa-btn { min-height: 36px; font-size: 12px; }
.favorites-grid { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 14px; }.favorite-card { border: 1px solid var(--border); border-radius: 11px; padding: 17px; display: flex; flex-direction: column; min-width: 0; }.favorite-heading { display: flex; gap: 12px; align-items: flex-start; position: relative; }.favorite-icon { display: grid; place-items: center; width: 48px; height: 48px; border-radius: 11px; color: #fff; flex-shrink: 0; }.favorite-icon--blue { background: #328ef1; }.favorite-icon--purple { background: #805bea; }.favorite-icon--green { background: #0dae69; }.favorite-icon--orange { background: #f39122; }.favorite-icon--pink { background: #ed507c; }.favorite-name { min-width: 0; flex: 1; }.favorite-name h2 { font-size: 14px; line-height: 1.6; overflow-wrap: anywhere; margin: 0; }.favorite-name p { margin: 5px 0 0; font-size: 11px; color: var(--muted); line-height: 1.7; }.favorite-heading .pa-badge { position: absolute; top: 52px; left: 0; font-size: 10px; }
.favorite-version { display: flex; gap: 8px; align-items: center; margin-top: 28px; font-size: 12px; color: var(--muted); }.favorite-version > svg { flex-shrink: 0; }.favorite-version strong { margin-left: 10px; color: var(--text); font-size: 19px; overflow-wrap: anywhere; }.favorite-time { display: flex; gap: 8px; align-items: flex-start; color: var(--muted); font-size: 11px; margin: 12px 0; line-height: 1.6; }.favorite-time svg { flex-shrink: 0; }.favorite-error, .favorite-info { font-size: 11px; padding: 7px 9px; margin: 0 0 12px; }.favorite-error small { display: block; margin-top: 5px; font-size: 10px; }.favorite-actions { display: flex; flex-wrap: wrap; gap: 7px; margin-top: auto; padding-top: 4px; }.favorite-actions .pa-btn { font-size: 11px; padding: 7px 9px; min-height: 36px; }.favorite-actions .favorite-remove { margin-left: auto; padding: 7px; }.favorites-grid--list { grid-template-columns: minmax(0, 1fr); }.favorites-grid--list .favorite-card { display: grid; grid-template-columns: minmax(240px, 1.2fr) minmax(200px, 1fr) auto; align-items: center; gap: 14px; }.favorites-grid--list .favorite-heading .pa-badge { position: static; }.favorites-grid--list .favorite-version { margin-top: 0; }.favorites-grid--list .favorite-time { grid-column: 1 / 3; margin: 0; }.favorites-grid--list .favorite-actions { grid-column: 3; grid-row: 1 / 3; }.favorites-grid--list .favorite-error, .favorites-grid--list .favorite-info { grid-column: 1 / -1; margin: 0; }
.favorites-note, .favorites-working { margin: 16px 0 0; font-size: 11px; color: var(--muted); line-height: 1.7; }.favorites-working { margin: 0 0 12px; }
@media (max-width: 1200px) { .favorites-grid { grid-template-columns: repeat(2, minmax(0, 1fr)); }.favorites-summary { grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 18px; }.favorites-check-all { grid-column: 1 / -1; flex-direction: row; align-items: center; }.favorites-check-all button { min-width: 210px; }.favorites-stat:last-of-type { border: 0; }.favorites-grid--list { grid-template-columns: minmax(0, 1fr); }.favorites-grid--list .favorite-card { display: flex; align-items: stretch; }.favorites-grid--list .favorite-heading .pa-badge { position: static; }.favorites-grid--list .favorite-actions { margin-top: 0; }.favorites-grid--list .favorite-version { margin-top: 4px; } }
@media (max-width: 680px) { .favorites-add { flex-wrap: wrap; gap: 10px; }.favorites-add .pa-input-group { flex-basis: 100%; }.favorites-add > button { width: 100%; }.favorites-summary { gap: 14px; padding: 16px; }.favorites-stat { gap: 8px; flex-direction: column; }.favorites-stat > svg { width: 25px; }.favorites-stat strong { font-size: 25px; }.favorites-stat small { font-size: 10px; }.favorites-check-all { align-items: flex-start; flex-direction: column; }.favorites-check-all button { width: 100%; }.favorites-grid { grid-template-columns: minmax(0, 1fr); }.favorites-display { flex-wrap: wrap; }.favorite-heading .pa-badge { position: static; }.favorite-version { margin-top: 18px; } }
.favorite-heading { flex-wrap: wrap; }.favorite-heading .pa-badge { position: static; margin-left: auto; align-self: flex-start; }.favorite-name { flex-basis: calc(100% - 60px); }.favorite-heading > .pa-badge { margin-top: -3px; }.favorite-version { margin-top: 12px; }.favorite-name h2 { font-size: 15px; }.favorite-name p { font-size: 12px; }.favorite-card { padding: 18px; }.favorite-actions .pa-btn { font-size: 12px; }.favorite-time { font-size: 12px; }.favorites-grid--list .favorite-name { flex-basis: auto; }
.favorite-heading { display: grid; grid-template-columns: 48px minmax(0, 1fr); }.favorite-heading > .pa-badge { position: absolute; top: 0; right: 0; left: auto; margin: 0; }.favorite-name h2 { padding-right: 70px; }.favorite-version { margin-top: 18px; }.favorites-grid--list .favorite-heading { display: flex; }.favorites-grid--list .favorite-heading > .pa-badge { position: static; }.favorites-grid--list .favorite-name h2 { padding-right: 0; }
</style>
