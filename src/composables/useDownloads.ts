import { onMounted, onUnmounted, ref } from 'vue'
import { downloadsApi, errorMessage } from '../services/downloads'
import type { Snapshot, TaskAction } from '../types'
import { defaultSettings } from '../types'

export function useDownloads() {
  const snapshot = ref<Snapshot>({ tasks: [], lastDirectory: null, error: null, revision: -1, settings: defaultSettings(), queueRevision: 0, diagnostics: [], diagnosing: false, notices: [], favorites: [] })
  const available = downloadsApi.available()
  const loading = ref(available)
  const error = ref('')
  const pending = ref(new Set<string>())
  let disposed = false
  let unlisten: (() => void) | undefined
  let timer: ReturnType<typeof setInterval> | undefined
  let refreshing = false

  function apply(next: Snapshot) {
    if (!disposed && next.revision >= snapshot.value.revision) snapshot.value = next
  }

  async function refresh() {
    if (refreshing || disposed) return
    refreshing = true
    try { apply(await downloadsApi.load()) }
    catch (cause) { if (!disposed) error.value = errorMessage(cause) }
    finally { refreshing = false; if (!disposed) loading.value = false }
  }

  async function perform(key: string, work: () => Promise<Snapshot | void>) {
    if (pending.value.has(key)) return false
    pending.value = new Set([...pending.value, key])
    error.value = ''
    try {
      const next = await work()
      if (next) apply(next)
      return true
    } catch (cause) {
      if (!disposed) error.value = errorMessage(cause)
      return false
    } finally {
      const remaining = new Set(pending.value)
      remaining.delete(key)
      pending.value = remaining
    }
  }

  onMounted(async () => {
    if (!available) return
    try {
      const stop = await downloadsApi.subscribe(apply)
      if (disposed) { stop(); return }
      unlisten = stop
      await refresh()
      if (!disposed) timer = setInterval(refresh, 5000)
    } catch (cause) {
      if (!disposed) { error.value = errorMessage(cause); loading.value = false }
    }
  })

  onUnmounted(() => { disposed = true; unlisten?.(); if (timer) clearInterval(timer) })

  return {
    snapshot, available, loading, error, pending, refresh, apply, perform,
    create: (url: string, directory: string) => perform('create', () => downloadsApi.create(url, directory)),
    act: (id: string, action: TaskAction) => perform(id, () => downloadsApi.act(id, action)),
    openDirectory: (id: string) => perform(id, () => downloadsApi.openDirectory(id)),
  }
}
