import { mount, flushPromises } from '@vue/test-utils'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import App from './App.vue'
import type { DownloadTask, Snapshot } from './types'
import { downloadsApi } from './services/downloads'

vi.mock('./services/downloads', () => ({
  downloadsApi: { available: vi.fn(() => true), load: vi.fn(), create: vi.fn(), act: vi.fn(), openDirectory: vi.fn(), chooseDirectory: vi.fn(), subscribe: vi.fn() },
  errorMessage: (error: unknown) => String(error),
}))

const task = (status: DownloadTask['status'] = 'paused'): DownloadTask => ({
  id: 'one', url: 'https://github.com/test/repo/releases/download/v1/file.exe', filename: '中文安装程序.exe',
  directory: 'F:\\下载 目录', status, downloaded: 1024, total: 4096, speed: 1024, eta: 3,
  route: 'GitHub 直连', verification: 'pending', error: null, finalPath: null, createdAt: 1, revision: 1,
})
const snapshot = (tasks: DownloadTask[] = [], revision = 1): Snapshot => ({ tasks, lastDirectory: 'F:\\下载 目录', error: null, revision, settings: { limitKib: 0, closeToTray: false, autoCheck: false }, queueRevision: 0, diagnostics: [], diagnosing: false, notices: [], favorites: [] })
let callback: (value: Snapshot) => void
let stop = vi.fn<() => void>()
const wrappers: ReturnType<typeof mount>[] = []
function render() { const wrapper = mount(App, { attachTo: document.body }); wrappers.push(wrapper); return wrapper }

beforeEach(() => {
  vi.clearAllMocks()
  vi.useFakeTimers()
  vi.mocked(downloadsApi.available).mockReturnValue(true)
  vi.mocked(downloadsApi.load).mockResolvedValue(snapshot())
  stop = vi.fn()
  vi.mocked(downloadsApi.subscribe).mockImplementation(async handler => { callback = handler; return stop })
})
afterEach(() => { wrappers.splice(0).forEach(wrapper => wrapper.unmount()); document.body.innerHTML = ''; vi.useRealTimers() })

describe('下载工作台', () => {
  it('保留上次目录，提交真实 API，并防止重复提交', async () => {
    const wrapper = render()
    await flushPromises()
    expect((wrapper.get('#save-directory').element as HTMLInputElement).value).toBe('F:\\下载 目录')
    await wrapper.get('#resource-url').setValue(task().url)
    let finish!: (value: Snapshot) => void
    vi.mocked(downloadsApi.create).mockReturnValue(new Promise(resolve => { finish = resolve }))
    await wrapper.get('form').trigger('submit')
    await wrapper.get('form').trigger('submit')
    expect(downloadsApi.create).toHaveBeenCalledTimes(1)
    expect(wrapper.get('.app-submit').attributes('disabled')).toBeDefined()
    finish(snapshot([task('queued')], 2))
    await flushPromises()
    expect((wrapper.get('#resource-url').element as HTMLInputElement).value).toBe('')
    expect(wrapper.text()).toContain('等待下载')
  })

  it('拒绝旧快照覆盖暂停状态，并清理订阅和轮询', async () => {
    const wrapper = render()
    await flushPromises()
    callback(snapshot([task('paused')], 5))
    callback(snapshot([task('downloading')], 4))
    await flushPromises()
    expect(wrapper.text()).toContain('已暂停')
    expect(wrapper.text()).not.toContain('正在下载')
    wrapper.unmount()
    wrappers.splice(wrappers.indexOf(wrapper), 1)
    expect(stop).toHaveBeenCalledOnce()
    const calls = vi.mocked(downloadsApi.load).mock.calls.length
    await vi.advanceTimersByTimeAsync(6000)
    expect(downloadsApi.load).toHaveBeenCalledTimes(calls)
  })

  it('区分未经官方校验的完成结果，并展示失败原因和重试入口', async () => {
    const finished = { ...task('completed'), verification: 'unverified' as const }
    const failed = { ...task('failed'), id: 'two', error: '下载服务返回 HTTP 404' }
    vi.mocked(downloadsApi.load).mockResolvedValue(snapshot([finished, failed]))
    const wrapper = render()
    await flushPromises()
    expect(wrapper.text()).toContain('未通过官方摘要验证')
    expect(wrapper.text()).toContain('HTTP 404')
    vi.mocked(downloadsApi.act).mockResolvedValue(snapshot([finished, { ...failed, status: 'queued' }], 2))
    await wrapper.get('[aria-label="重试下载"]').trigger('click')
    await flushPromises()
    expect(downloadsApi.act).toHaveBeenCalledWith('two', 'resume')
  })

  it('浏览器预览不伪造下载或目录选择', async () => {
    vi.mocked(downloadsApi.available).mockReturnValue(false)
    const wrapper = render()
    await flushPromises()
    expect(wrapper.text()).toContain('当前是浏览器预览')
    expect(wrapper.get('#resource-url').attributes('disabled')).toBeDefined()
    expect(downloadsApi.load).not.toHaveBeenCalled()
  })

  it('目录选择失败有反馈，任务不会被创建', async () => {
    const wrapper = render()
    await flushPromises()
    vi.mocked(downloadsApi.chooseDirectory).mockRejectedValue('目录不可访问')
    await wrapper.get('.app-directory__control button').trigger('click')
    await flushPromises()
    expect(wrapper.get('[role="alert"]').text()).toContain('目录不可访问')
    expect(downloadsApi.create).not.toHaveBeenCalled()
  })
})
