import { mount, flushPromises } from '@vue/test-utils'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import App from './App.vue'
import type { DownloadTask, Snapshot } from './types'
import { downloadsApi } from './services/downloads'

vi.mock('./services/downloads', () => ({
  downloadsApi: { available: vi.fn(() => true), load: vi.fn(), create: vi.fn(), act: vi.fn(), openDirectory: vi.fn(), chooseDirectory: vi.fn(), subscribe: vi.fn(), inspectDirectory: vi.fn(), createDirectory: vi.fn(), previewBatch: vi.fn(), createBatch: vi.fn(), browse: vi.fn() },
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
function render() {
  const wrapper = mount(App, { attachTo: document.body })
  const dialog = wrapper.get('.app-directory-dialog').element as HTMLDialogElement
  dialog.showModal = () => dialog.setAttribute('open', '')
  dialog.close = () => { dialog.removeAttribute('open'); dialog.dispatchEvent(new Event('close')) }
  wrappers.push(wrapper)
  return wrapper
}
async function click(wrapper: ReturnType<typeof mount>, label: string) {
  const button = wrapper.findAll('button').find(item => item.text() === label)
  expect(button, label).toBeDefined()
  await button!.trigger('click')
  await flushPromises()
}

beforeEach(() => {
  vi.resetAllMocks()
  vi.useFakeTimers()
  vi.mocked(downloadsApi.available).mockReturnValue(true)
  vi.mocked(downloadsApi.load).mockResolvedValue(snapshot())
  vi.mocked(downloadsApi.inspectDirectory).mockImplementation(async directory => ({ directory, state: 'existing' }))
  vi.mocked(downloadsApi.createDirectory).mockImplementation(async directory => directory)
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
    await flushPromises()
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

  it('手输缺失目录先确认，只创建一次并继续原下载', async () => {
    const wrapper = render(); await flushPromises()
    const directory = 'F:\\新的下载\\项目'
    vi.mocked(downloadsApi.inspectDirectory).mockResolvedValue({ directory, state: 'missing' })
    vi.mocked(downloadsApi.create).mockResolvedValue(snapshot([task('queued')], 2))
    await wrapper.get('#save-directory').setValue(directory)
    await wrapper.get('#resource-url').setValue(task().url)
    expect(downloadsApi.inspectDirectory).not.toHaveBeenCalled()
    await wrapper.get('form').trigger('submit'); await flushPromises()
    await wrapper.get('form').trigger('submit')
    expect(wrapper.get('.app-directory-dialog').attributes('open')).toBeDefined()
    expect(wrapper.get('.app-directory-dialog').text()).toContain(directory)
    expect(downloadsApi.inspectDirectory).toHaveBeenCalledOnce()
    expect(downloadsApi.createDirectory).not.toHaveBeenCalled()
    expect(downloadsApi.create).not.toHaveBeenCalled()
    await click(wrapper, '新建并继续')
    expect(downloadsApi.createDirectory).toHaveBeenCalledExactlyOnceWith(directory)
    expect(downloadsApi.create).toHaveBeenCalledExactlyOnceWith(task().url, directory)
    expect((wrapper.get('#resource-url').element as HTMLInputElement).value).toBe('')
  })

  it.each(['取消', 'cancel', 'close'])('通过 %s 关闭新建确认不创建目录或任务，并保留输入', async method => {
    const wrapper = render(); await flushPromises()
    vi.mocked(downloadsApi.inspectDirectory).mockResolvedValue({ directory: 'F:\\missing', state: 'missing' })
    await wrapper.get('#resource-url').setValue(task().url)
    await wrapper.get('form').trigger('submit'); await flushPromises()
    if (method === '取消') await click(wrapper, method)
    else if (method === 'close') (wrapper.get('.app-directory-dialog').element as HTMLDialogElement).close()
    else await wrapper.get('.app-directory-dialog').trigger('cancel')
    await flushPromises()
    expect(downloadsApi.createDirectory).not.toHaveBeenCalled()
    expect(downloadsApi.create).not.toHaveBeenCalled()
    expect((wrapper.get('#resource-url').element as HTMLInputElement).value).toBe(task().url)
    expect(wrapper.get('.app-submit').attributes('disabled')).toBeUndefined()
    expect(document.activeElement?.id).toBe('save-directory')
  })

  it('创建失败展示原因并保留路径和链接', async () => {
    const wrapper = render(); await flushPromises()
    vi.mocked(downloadsApi.inspectDirectory).mockResolvedValue({ directory: 'F:\\missing', state: 'missing' })
    vi.mocked(downloadsApi.createDirectory).mockRejectedValue('创建目录失败：拒绝访问')
    await wrapper.get('#resource-url').setValue(task().url)
    await wrapper.get('form').trigger('submit'); await flushPromises()
    await click(wrapper, '新建并继续')
    expect(wrapper.get('[role="alert"]').text()).toContain('创建目录失败：拒绝访问')
    expect(downloadsApi.create).not.toHaveBeenCalled()
    expect((wrapper.get('#resource-url').element as HTMLInputElement).value).toBe(task().url)
    expect((wrapper.get('#save-directory').element as HTMLInputElement).value).toBe(snapshot().lastDirectory)
  })

  it('不可访问目录不弹出新建确认', async () => {
    const wrapper = render(); await flushPromises()
    vi.mocked(downloadsApi.inspectDirectory).mockRejectedValue('无法访问保存目录，请检查目录权限')
    await wrapper.get('#resource-url').setValue(task().url)
    await wrapper.get('form').trigger('submit'); await flushPromises()
    expect(wrapper.get('[role="alert"]').text()).toContain('检查目录权限')
    expect(wrapper.get('.app-directory-dialog').attributes('open')).toBeUndefined()
    expect(downloadsApi.createDirectory).not.toHaveBeenCalled()
    expect(downloadsApi.create).not.toHaveBeenCalled()
  })

  it('浏览取消保留原目录，选择过的目录消失后要求重新选择', async () => {
    const wrapper = render(); await flushPromises()
    vi.mocked(downloadsApi.chooseDirectory).mockResolvedValueOnce(null).mockResolvedValueOnce('F:\\chosen')
    await click(wrapper, '浏览')
    expect((wrapper.get('#save-directory').element as HTMLInputElement).value).toBe(snapshot().lastDirectory)
    await click(wrapper, '浏览')
    expect((wrapper.get('#save-directory').element as HTMLInputElement).value).toBe('F:\\chosen')
    vi.mocked(downloadsApi.inspectDirectory).mockResolvedValue({ directory: 'F:\\chosen', state: 'missing' })
    await wrapper.get('#resource-url').setValue(task().url)
    await wrapper.get('form').trigger('submit'); await flushPromises()
    expect(wrapper.get('[role="alert"]').text()).toContain('重新选择目录')
    expect(downloadsApi.createDirectory).not.toHaveBeenCalled()
    expect(downloadsApi.create).not.toHaveBeenCalled()
    await wrapper.get('#save-directory').setValue('F:\\manual')
    await wrapper.get('form').trigger('submit'); await flushPromises()
    expect(wrapper.get('.app-directory-dialog').attributes('open')).toBeDefined()
  })

  it('目录检查尚未结束时修改路径使旧响应失效', async () => {
    const wrapper = render(); await flushPromises()
    let finish!: (value: Awaited<ReturnType<typeof downloadsApi.inspectDirectory>>) => void
    vi.mocked(downloadsApi.inspectDirectory).mockReturnValue(new Promise(resolve => { finish = resolve }))
    await wrapper.get('#resource-url').setValue(task().url)
    await wrapper.get('form').trigger('submit')
    await wrapper.get('#save-directory').setValue('F:\\new')
    finish({ directory: 'F:\\old', state: 'missing' }); await flushPromises()
    expect(wrapper.get('.app-directory-dialog').attributes('open')).toBeUndefined()
    expect(downloadsApi.createDirectory).not.toHaveBeenCalled()
    expect(downloadsApi.create).not.toHaveBeenCalled()
  })

  it('确认期间切换页面取消待执行的新建操作', async () => {
    const wrapper = render(); await flushPromises()
    vi.mocked(downloadsApi.inspectDirectory).mockResolvedValue({ directory: 'F:\\missing', state: 'missing' })
    await wrapper.get('#resource-url').setValue(task().url)
    await wrapper.get('form').trigger('submit'); await flushPromises()
    await click(wrapper, '设置')
    expect(wrapper.get('.app-directory-dialog').attributes('open')).toBeUndefined()
    expect(downloadsApi.createDirectory).not.toHaveBeenCalled()
    expect(downloadsApi.create).not.toHaveBeenCalled()
  })

  it('创建期间重复点击和组件卸载不会继续添加任务', async () => {
    const wrapper = render(); await flushPromises()
    vi.mocked(downloadsApi.inspectDirectory).mockResolvedValue({ directory: 'F:\\missing', state: 'missing' })
    let finish!: (value: string) => void
    vi.mocked(downloadsApi.createDirectory).mockReturnValue(new Promise(resolve => { finish = resolve }))
    await wrapper.get('#resource-url').setValue(task().url)
    await wrapper.get('form').trigger('submit'); await flushPromises()
    await click(wrapper, '新建并继续')
    await click(wrapper, '新建并继续')
    expect(downloadsApi.createDirectory).toHaveBeenCalledOnce()
    wrapper.unmount(); wrappers.splice(wrappers.indexOf(wrapper), 1)
    finish('F:\\missing'); await flushPromises()
    expect(downloadsApi.create).not.toHaveBeenCalled()
  })

  it('确认弹框未响应时卸载也会结束等待，不创建目录', async () => {
    const wrapper = render(); await flushPromises()
    vi.mocked(downloadsApi.inspectDirectory).mockResolvedValue({ directory: 'F:\\missing', state: 'missing' })
    await wrapper.get('#resource-url').setValue(task().url)
    await wrapper.get('form').trigger('submit'); await flushPromises()
    wrapper.unmount(); wrappers.splice(wrappers.indexOf(wrapper), 1)
    await flushPromises()
    expect(downloadsApi.createDirectory).not.toHaveBeenCalled()
    expect(downloadsApi.create).not.toHaveBeenCalled()
  })

  it('批量预览共用新建确认，最终创建前再次检查目录', async () => {
    const wrapper = render(); await flushPromises()
    const directory = snapshot().lastDirectory!
    vi.mocked(downloadsApi.inspectDirectory).mockResolvedValueOnce({ directory, state: 'missing' }).mockResolvedValue({ directory, state: 'existing' })
    vi.mocked(downloadsApi.previewBatch).mockResolvedValue({ items: [{ input: task().url, url: task().url, filename: 'file.exe', size: 1024, status: 'valid', message: null, taskId: null }], knownSize: 1024, unknownCount: 0, preflight: { directory, available: 4096, required: 2048, warning: null } })
    vi.mocked(downloadsApi.createBatch).mockResolvedValue({ items: [], snapshot: snapshot([], 2) })
    await wrapper.get('#project-source').setValue(task().url)
    await click(wrapper, '预览批量链接')
    expect(downloadsApi.previewBatch).not.toHaveBeenCalled()
    await click(wrapper, '新建并继续')
    expect(downloadsApi.previewBatch).toHaveBeenCalledExactlyOnceWith([task().url], directory)
    expect(downloadsApi.createBatch).not.toHaveBeenCalled()
    await click(wrapper, '确认创建有效任务')
    expect(downloadsApi.inspectDirectory).toHaveBeenCalledTimes(2)
    expect(downloadsApi.createBatch).toHaveBeenCalledExactlyOnceWith([task().url], directory, null)
  })

  it('批量预览后目录消失会重新确认，取消不创建任务', async () => {
    const wrapper = render(); await flushPromises()
    const directory = snapshot().lastDirectory!
    vi.mocked(downloadsApi.previewBatch).mockResolvedValue({ items: [{ input: task().url, url: task().url, filename: 'file.exe', size: 1024, status: 'valid', message: null, taskId: null }], knownSize: 1024, unknownCount: 0, preflight: { directory, available: 4096, required: 2048, warning: null } })
    await wrapper.get('#project-source').setValue(task().url)
    await click(wrapper, '预览批量链接')
    vi.mocked(downloadsApi.inspectDirectory).mockResolvedValue({ directory, state: 'missing' })
    await click(wrapper, '确认创建有效任务')
    expect(wrapper.get('.app-directory-dialog').attributes('open')).toBeDefined()
    await click(wrapper, '取消')
    expect(downloadsApi.createDirectory).not.toHaveBeenCalled()
    expect(downloadsApi.createBatch).not.toHaveBeenCalled()
    expect(wrapper.find('[aria-label="批量预览"]').exists()).toBe(true)
  })

  it('浏览仓库版本不检查或创建保存目录', async () => {
    const wrapper = render(); await flushPromises()
    vi.mocked(downloadsApi.browse).mockResolvedValue({ repository: 'test/repo', releases: [], page: 0, hasMore: false, selectedUrl: null })
    await wrapper.get('#resource-url').setValue('https://github.com/test/repo')
    await wrapper.get('form').trigger('submit'); await flushPromises()
    expect(downloadsApi.browse).toHaveBeenCalledWith('https://github.com/test/repo', 0, false)
    expect(downloadsApi.inspectDirectory).not.toHaveBeenCalled()
    expect(downloadsApi.createDirectory).not.toHaveBeenCalled()
  })
})
