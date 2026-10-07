import { flushPromises, mount } from '@vue/test-utils'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import RouteFeedback from './components/RouteFeedback.vue'
import TaskRow from './components/TaskRow.vue'
import { downloadsApi } from './services/downloads'
import { defaultSettings, type DownloadTask, type Snapshot } from './types'

vi.mock('./services/downloads', () => ({ downloadsApi: { applySuggestion: vi.fn(), dismissSuggestion: vi.fn() }, errorMessage: (error: unknown) => String(error) }))
const wrappers: ReturnType<typeof mount>[] = []
const task = (overrides: Partial<DownloadTask> = {}): DownloadTask => ({
  id: 'task-one', url: 'https://github.com/test/repo/releases/download/v1/file.zip', filename: '下载文件.zip', directory: 'F:\\下载',
  status: 'downloading', downloaded: 20_000_000, total: 100_000_000, speed: 100_000, eta: 800, route: 'GitHub 直连',
  verification: 'pending', error: null, finalPath: null, createdAt: 1, revision: 1, ...overrides,
})
const suggested = (): DownloadTask => task({ routeSuggestion: { id: 'suggestion-one', routeId: 'gh-proxy', routeName: 'GH-Proxy', currentSeconds: 800, suggestedSeconds: 110, restartBytes: 20_000_000 } })
const snapshot = (): Snapshot => ({ tasks: [], lastDirectory: null, error: null, revision: 2, settings: defaultSettings(), queueRevision: 0, diagnostics: [], diagnosing: false, notices: [], favorites: [] })
function feedback(value: DownloadTask) {
  const wrapper = mount(RouteFeedback, { props: { task: value, busy: false } }); wrappers.push(wrapper); return wrapper
}
beforeEach(() => {
  vi.resetAllMocks()
  Object.defineProperty(HTMLDialogElement.prototype, 'showModal', { configurable: true, value: function (this: HTMLDialogElement) { this.setAttribute('open', '') } })
  Object.defineProperty(HTMLDialogElement.prototype, 'close', { configurable: true, value: function (this: HTMLDialogElement) { this.removeAttribute('open') } })
  vi.mocked(downloadsApi.applySuggestion).mockResolvedValue(snapshot())
  vi.mocked(downloadsApi.dismissSuggestion).mockResolvedValue(snapshot())
})
afterEach(() => wrappers.splice(0).forEach(wrapper => wrapper.unmount()))

describe('恢复和重试反馈', () => {
  it('展示后端倒计时和剩余额度，让位时解释计时暂停', async () => {
    const value = task({ status: 'waiting_network', recoveryInfo: { remainingMs: 290_000, retryInMs: 9_500, waitingForSlot: false } })
    const wrapper = feedback(value)
    expect(wrapper.text()).toContain('10 秒后重新检测')
    expect(wrapper.text()).toContain('剩余自动恢复额度 4 分 50 秒')
    expect(wrapper.text()).toContain('已保留当前进度')
    await wrapper.get('button').trigger('click')
    expect(wrapper.emitted('retry')).toHaveLength(1)
    await wrapper.setProps({ task: { ...value, recoveryInfo: { remainingMs: 290_000, retryInMs: 9_500, waitingForSlot: true } } })
    expect(wrapper.text()).toContain('恢复计时已暂停')
    expect(wrapper.text()).not.toContain('10 秒后重新检测')
  })

  it('重试显示次数和原因，不把中间状态显示为最终失败', () => {
    const wrapper = feedback(task({ status: 'retrying', retryInfo: { phase: 'retrying', attempt: 2, maxAttempts: 3, reason: '连接中断', retryInMs: 1_100 } }))
    expect(wrapper.text()).toContain('连接中断，2 秒后重试（第 2/3 次）')
    expect(wrapper.text()).not.toContain('下载失败')
  })

  it('等待恢复任务保留暂停和取消入口，速度不显示旧值', async () => {
    const wrapper = mount(TaskRow, { props: { task: task({ status: 'waiting_network', recoveryInfo: { remainingMs: 300_000, retryInMs: 10_000, waitingForSlot: false } }), busy: false } }); wrappers.push(wrapper)
    expect(wrapper.get('.task-speed').text()).toBe('—')
    await wrapper.get('[aria-label="暂停下载"]').trigger('click')
    expect(wrapper.emitted('action')?.[0]).toEqual(['task-one', 'pause'])
    await wrapper.get('[aria-label="取消下载"]').trigger('click')
    expect(wrapper.emitted('action')?.[1]).toEqual(['task-one', 'cancel'])
    await wrapper.setProps({ busy: true })
    expect(wrapper.get('[aria-label="暂停下载"]').attributes('disabled')).toBeDefined()
  })

  it('列表仅显示错误摘要，详情保留各线路原因', async () => {
    const value = task({ status: 'failed', error: '所有线路均未完成下载。GitHub：线路探测超时；GH-Proxy：HTTP 503', routeFailures: [{ routeId: 'github', routeName: 'GitHub', message: '线路探测超时', temporary: true }, { routeId: 'gh-proxy', routeName: 'GH-Proxy', message: 'HTTP 503', temporary: true }] })
    const wrapper = mount(TaskRow, { props: { task: value, busy: false } }); wrappers.push(wrapper)
    expect(wrapper.get('.task-name .pa-error').text()).toBe('所有线路均未完成下载')
    await wrapper.get('[title="下载详情与更多操作"]').trigger('click')
    expect(wrapper.get('[aria-label="各线路失败原因"]').text()).toContain('HTTP 503')
  })
})

describe('低速换线建议', () => {
  it('先展示进度损失，确认后才提交；确认中防止重复请求', async () => {
    let finish!: (value: Snapshot) => void
    vi.mocked(downloadsApi.applySuggestion).mockImplementation(() => new Promise(resolve => { finish = resolve }))
    const wrapper = feedback(suggested())
    expect(wrapper.text()).toContain('20.0 MB')
    expect(wrapper.text()).toContain('当前下载继续进行')
    await wrapper.get('.route-feedback__suggestion .pa-btn--primary').trigger('click')
    expect(wrapper.get('dialog').attributes('open')).toBeDefined()
    expect(downloadsApi.applySuggestion).not.toHaveBeenCalled()
    await wrapper.get('dialog .pa-btn--primary').trigger('click')
    await wrapper.get('dialog .pa-btn--primary').trigger('click')
    expect(downloadsApi.applySuggestion).toHaveBeenCalledTimes(1)
    expect(downloadsApi.applySuggestion).toHaveBeenCalledWith('task-one', 'suggestion-one')
    expect(wrapper.text()).toContain('正在复核线路')
    finish(snapshot()); await flushPromises()
    expect(wrapper.emitted('snapshot')).toHaveLength(1)
    expect(wrapper.get('dialog').attributes('open')).toBeUndefined()
  })

  it('忽略建议只提交忽略请求，保留当前下载', async () => {
    const wrapper = feedback(suggested())
    await wrapper.get('.route-feedback__suggestion .pa-btn:not(.pa-btn--primary)').trigger('click')
    await flushPromises()
    expect(downloadsApi.dismissSuggestion).toHaveBeenCalledWith('task-one', 'suggestion-one')
    expect(downloadsApi.applySuggestion).not.toHaveBeenCalled()
  })

  it('确认弹窗中的建议过期后禁止提交', async () => {
    const wrapper = feedback(suggested())
    await wrapper.get('.route-feedback__suggestion .pa-btn--primary').trigger('click')
    await wrapper.setProps({ task: task({ status: 'paused' }) })
    expect(wrapper.get('dialog').text()).toContain('建议已失效')
    expect(wrapper.get('dialog .pa-btn--primary').attributes('disabled')).toBeDefined()
    await wrapper.get('dialog .pa-btn--primary').trigger('click')
    expect(downloadsApi.applySuggestion).not.toHaveBeenCalled()
  })

  it('复核失败展示错误且不伪造进度归零', async () => {
    vi.mocked(downloadsApi.applySuggestion).mockRejectedValue('备选线路当前不可用，已保留原线路下载')
    const wrapper = feedback(suggested())
    await wrapper.get('.route-feedback__suggestion .pa-btn--primary').trigger('click')
    await wrapper.get('dialog .pa-btn--primary').trigger('click')
    await flushPromises()
    expect(wrapper.get('[role="alert"]').text()).toContain('已保留原线路下载')
    expect(wrapper.emitted('snapshot')).toBeUndefined()
    expect(wrapper.props('task').downloaded).toBe(20_000_000)
  })

  it('卸载后忽略未完成调用的回包', async () => {
    let finish!: (value: Snapshot) => void
    vi.mocked(downloadsApi.dismissSuggestion).mockImplementation(() => new Promise(resolve => { finish = resolve }))
    const wrapper = feedback(suggested())
    await wrapper.get('.route-feedback__suggestion .pa-btn:not(.pa-btn--primary)').trigger('click')
    wrapper.unmount(); wrappers.splice(wrappers.indexOf(wrapper), 1)
    finish(snapshot()); await flushPromises()
    expect(wrapper.emitted('snapshot')).toBeUndefined()
  })
})
