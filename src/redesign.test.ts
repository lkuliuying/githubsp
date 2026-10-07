import { mount, flushPromises } from '@vue/test-utils'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import App from './App.vue'
import SourcePicker from './components/SourcePicker.vue'
import HistoryView from './components/HistoryView.vue'
import FavoritesView from './components/FavoritesView.vue'
import SettingsView from './components/SettingsView.vue'
import RouteDiagnostics from './components/RouteDiagnostics.vue'
import type { CatalogPage, DownloadTask, Favorite, Snapshot } from './types'
import { defaultSettings } from './types'
import { downloadsApi } from './services/downloads'

vi.mock('./services/downloads', () => ({
  downloadsApi: { available: vi.fn(() => true), load: vi.fn(), subscribe: vi.fn(), act: vi.fn(), create: vi.fn(), browse: vi.fn(), previewBatch: vi.fn(), history: vi.fn(), copy: vi.fn(), openDirectory: vi.fn(), saveSettings: vi.fn(), checkFavorites: vi.fn(), removeFavorite: vi.fn(), favorite: vi.fn() },
  errorMessage: (value: unknown) => String(value),
}))

const makeTask = (id: string, status: DownloadTask['status']): DownloadTask => ({
  id, status, url: 'https://github.com/test/repo/releases/download/v1/' + id + '.zip', filename: id + '.zip',
  directory: 'F:\\下载', downloaded: 1024, total: 4096, speed: 0, eta: null, route: 'GitHub 直连',
  verification: 'pending', error: null, finalPath: null, createdAt: 1, revision: 1,
})
const makeSnapshot = (tasks: DownloadTask[] = []): Snapshot => ({ tasks, lastDirectory: 'F:\\下载', error: null, revision: 1, settings: defaultSettings(), queueRevision: 0, diagnostics: [], diagnosing: false, notices: [], favorites: [] })
const makeCatalog = (page: number, hasMore = false): CatalogPage => ({
  repository: 'test/repo', page, hasMore, selectedUrl: null,
  releases: [{ id: page + 1, tag: `v${page + 1}`, name: '', prerelease: false, url: '', notes: '', assets: [
    { id: page * 10 + 1, name: `page-${page}.exe`, size: 1024, url: `https://github.com/test/repo/releases/download/v${page + 1}/page-${page}.exe`, sha256: null, hints: ['Windows'] },
  ] }],
})
function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (reason: unknown) => void
  const promise = new Promise<T>((done, fail) => { resolve = done; reject = fail })
  return { promise, resolve, reject }
}
const wrappers: ReturnType<typeof mount>[] = []
function render(component: Parameters<typeof mount>[0], props: Record<string, unknown> = {}) {
  const wrapper = mount(component, { props: { ...(component === SourcePicker ? { prepareDirectory: async (directory: string) => directory.trim() } : {}), ...props }, attachTo: document.body }); wrappers.push(wrapper); return wrapper
}
async function click(wrapper: ReturnType<typeof mount>, label: string) {
  const button = wrapper.findAll('button').find(item => item.text().includes(label))
  expect(button, label).toBeDefined(); await button!.trigger('click'); await flushPromises()
}
beforeEach(() => {
  vi.resetAllMocks()
  vi.mocked(downloadsApi.available).mockReturnValue(true)
  vi.mocked(downloadsApi.subscribe).mockResolvedValue(() => {})
  vi.mocked(downloadsApi.load).mockResolvedValue(makeSnapshot())
})
afterEach(() => { wrappers.splice(0).forEach(wrapper => wrapper.unmount()); document.body.innerHTML = ''; vi.restoreAllMocks() })

describe('参考布局重构的交互回归', () => {
  it('批量暂停只操作活动和等待任务，批量继续只操作暂停和失败任务', async () => {
    const tasks = [makeTask('active', 'downloading'), makeTask('queue', 'queued'), makeTask('paused', 'paused'), makeTask('done', 'completed'), makeTask('failure', 'failed')]
    const state = makeSnapshot(tasks)
    vi.mocked(downloadsApi.load).mockResolvedValue(state)
    vi.mocked(downloadsApi.act).mockImplementation(async (id, action) => {
      state.tasks = state.tasks.map(task => task.id === id ? { ...task, status: action === 'pause' ? 'paused' : 'queued' } : task)
      return { ...state, revision: ++state.revision }
    })
    const wrapper = render(App); await flushPromises()
    await click(wrapper, '全部暂停')
    expect(vi.mocked(downloadsApi.act).mock.calls).toEqual([['active', 'pause'], ['queue', 'pause']])
    vi.mocked(downloadsApi.act).mockClear()
    await click(wrapper, '全部继续')
    expect(vi.mocked(downloadsApi.act).mock.calls).toEqual([['active', 'resume'], ['queue', 'resume'], ['paused', 'resume'], ['failure', 'resume']])
  })

  it('批量操作失败后停止，错误保留在页面', async () => {
    vi.mocked(downloadsApi.load).mockResolvedValue(makeSnapshot([makeTask('one', 'paused'), makeTask('two', 'paused')]))
    vi.mocked(downloadsApi.act).mockRejectedValue('任务状态已变化，请刷新后重试')
    const wrapper = render(App); await flushPromises(); await click(wrapper, '全部继续')
    expect(downloadsApi.act).toHaveBeenCalledTimes(1)
    expect(wrapper.get('[role="alert"]').text()).toContain('任务状态已变化')
  })

  it('工作台卸载后不会继续提交剩余批量操作', async () => {
    vi.mocked(downloadsApi.load).mockResolvedValue(makeSnapshot([makeTask('one', 'paused'), makeTask('two', 'paused')]))
    let finish!: (value: Snapshot) => void
    vi.mocked(downloadsApi.act).mockReturnValue(new Promise(resolve => { finish = resolve }))
    const wrapper = render(App); await flushPromises(); await click(wrapper, '全部继续')
    expect(downloadsApi.act).toHaveBeenCalledTimes(1)
    wrapper.unmount(); wrappers.splice(wrappers.indexOf(wrapper), 1)
    finish({ ...makeSnapshot([makeTask('one', 'queued'), makeTask('two', 'paused')]), revision: 2 }); await flushPromises()
    expect(downloadsApi.act).toHaveBeenCalledTimes(1)
  })

  it('任务详情保留换线和排序入口，清空已完成需要明确确认', async () => {
    vi.mocked(downloadsApi.load).mockResolvedValue(makeSnapshot([makeTask('queue', 'queued'), makeTask('done', 'completed')]))
    vi.mocked(downloadsApi.act).mockResolvedValue({ ...makeSnapshot([makeTask('queue', 'queued')]), revision: 2 })
    const wrapper = render(App); await flushPromises()
    // jsdom 不实现原生对话框，仅给当前测试元素补充打开和关闭行为。
    const dialog = wrapper.get('dialog.app-dialog').element as HTMLDialogElement
    dialog.showModal = () => dialog.setAttribute('open', '')
    dialog.close = () => dialog.removeAttribute('open')
    await wrapper.get('[aria-label="下载详情：queue.zip"]').trigger('click')
    expect(wrapper.text()).toContain('下载线路'); expect(wrapper.text()).toContain('置顶')
    await click(wrapper, '清空已完成')
    expect(downloadsApi.act).not.toHaveBeenCalled()
    expect(wrapper.get('dialog.app-dialog').attributes('open')).toBeDefined()
    await click(wrapper, '清空记录，保留文件')
    expect(downloadsApi.act).toHaveBeenCalledExactlyOnceWith('done', 'remove')
  })

  it('仓库简称转换为官方地址，版本切换保留跨版本选择', async () => {
    const first = { id: 11, name: 'one.exe', size: 12, url: makeTask('one', 'queued').url, sha256: null, hints: [] }
    const second = { ...first, id: 12, name: 'two.exe', url: makeTask('two', 'queued').url }
    vi.mocked(downloadsApi.browse).mockResolvedValue({ repository: 'test/repo', page: 1, hasMore: false, selectedUrl: null, releases: [
      { id: 1, tag: 'v1', name: '', prerelease: false, url: '', notes: '', assets: [first] },
      { id: 2, tag: 'v2', name: '', prerelease: false, url: '', notes: '', assets: [second] },
    ] })
    const wrapper = render(SourcePicker, { directory: 'F:\\下载', ready: true })
    await wrapper.get('#repository-source').setValue('test/repo'); await click(wrapper, '查找版本')
    expect(downloadsApi.browse).toHaveBeenCalledWith('https://github.com/test/repo', 0, false)
    await wrapper.get('.asset input').setValue(true)
    await wrapper.findAll('.source-release')[1]!.trigger('click')
    expect(wrapper.text()).toContain('two.exe'); expect(wrapper.text()).toContain('已选 1 项')
    await wrapper.get('.asset input').setValue(true)
    await click(wrapper, '预览所选')
    expect(downloadsApi.previewBatch).toHaveBeenCalledWith([first.url, second.url], 'F:\\下载')
  })

  it('首次查询复用标题栏状态位，等待时拒绝重复提交', async () => {
    const request = deferred<CatalogPage>()
    vi.mocked(downloadsApi.browse).mockReturnValue(request.promise)
    const wrapper = render(SourcePicker, { directory: 'F:\\下载', ready: true })
    const status = wrapper.get('.section-heading [role="status"]').element
    expect(status.textContent).toBe('')
    await wrapper.get('#repository-source').setValue('test/repo')
    await wrapper.get('.source-search').trigger('submit')
    expect(wrapper.get('.source-status').element).toBe(status)
    expect(status.textContent).toContain('正在查找版本…')
    expect(wrapper.get('.source-browser').attributes('aria-busy')).toBe('true')
    expect(wrapper.get('#repository-source').attributes('disabled')).toBeDefined()
    expect(wrapper.find('.source-feedback').exists()).toBe(false)
    await wrapper.get('.source-search').trigger('submit')
    expect(downloadsApi.browse).toHaveBeenCalledExactlyOnceWith('https://github.com/test/repo', 0, false)
    request.resolve(makeCatalog(0)); await flushPromises()
    expect(wrapper.get('.source-status').element).toBe(status)
    expect(status.textContent).toBe('')
    expect(wrapper.get('.source-browser').attributes('aria-busy')).toBe('false')
    expect(wrapper.text()).toContain('浏览其他版本')
  })

  it('翻页等待保留列表、附件和选择，仅成功后更新页码并恢复控件', async () => {
    const request = deferred<CatalogPage>()
    vi.mocked(downloadsApi.browse).mockResolvedValueOnce(makeCatalog(1, true)).mockReturnValueOnce(request.promise)
    const wrapper = render(SourcePicker, { directory: 'F:\\下载', ready: true })
    await wrapper.get('#repository-source').setValue('test/repo'); await click(wrapper, '查找版本')
    await wrapper.get('.asset input').setValue(true)
    const releaseRow = wrapper.get('.source-release').element
    const assetRow = wrapper.get('.asset').element
    const status = wrapper.get('.source-status').element
    const next = wrapper.get('[aria-label="下一页版本"]')
    await next.trigger('click')
    expect(wrapper.get('.source-status').element).toBe(status)
    expect(status.textContent).toContain('正在加载第 2 页…')
    expect(wrapper.get('.source-pagination').text()).toContain('第 1 页')
    expect(wrapper.get('.source-release').element).toBe(releaseRow)
    expect(wrapper.get('.asset').element).toBe(assetRow)
    expect((wrapper.get('.asset input').element as HTMLInputElement).checked).toBe(true)
    expect(wrapper.get('.source-selection').text()).toContain('已选 1 项')
    expect(next.attributes('disabled')).toBeDefined()
    expect(next.classes()).toContain('source-pending-control')
    expect(wrapper.get('.source-selection button').classes()).toContain('source-pending-control')
    await next.trigger('click'); await wrapper.get('.source-search').trigger('submit')
    expect(downloadsApi.browse).toHaveBeenCalledTimes(2)
    expect(downloadsApi.browse).toHaveBeenLastCalledWith('https://github.com/test/repo', 2, false)
    request.resolve(makeCatalog(2)); await flushPromises()
    expect(wrapper.get('.source-pagination').text()).toContain('第 2 页')
    expect(wrapper.text()).toContain('page-2.exe')
    expect(wrapper.text()).not.toContain('page-1.exe')
    expect(wrapper.get('.source-selection').text()).toContain('已选 0 项')
    expect(status.textContent).toBe('')
    expect(wrapper.get('[aria-label="上一页版本"]').attributes('disabled')).toBeUndefined()
    expect(next.attributes('disabled')).toBeDefined()
    expect(next.classes()).not.toContain('source-pending-control')
    expect(wrapper.find('.source-pending-control').exists()).toBe(false)
  })

  it('加载期间首页、空选择、空批量和不可用附件继续保持不可用状态', async () => {
    const page = makeCatalog(1, true)
    page.releases[0]!.assets.push({ ...page.releases[0]!.assets[0]!, id: 99, name: 'blocked.exe', url: 'https://github.com/test/repo/releases/download/v2/blocked.exe', unavailable: '文件名不可用' })
    const request = deferred<CatalogPage>()
    vi.mocked(downloadsApi.browse).mockResolvedValueOnce(page).mockReturnValueOnce(request.promise)
    const wrapper = mount(SourcePicker, { props: { directory: 'F:\\下载', ready: true, prepareDirectory: async directory => directory }, attachTo: document.body }); wrappers.push(wrapper)
    await wrapper.get('#repository-source').setValue('test/repo'); await click(wrapper, '查找版本')
    await wrapper.get('[aria-label="下一页版本"]').trigger('click')
    for (const selector of ['[aria-label="上一页版本"]', '.source-selection button', '.source-batch button', '[aria-label="选择 blocked.exe"]']) {
      expect(wrapper.get(selector).attributes('disabled')).toBeDefined()
      expect(wrapper.get(selector).classes()).not.toContain('source-pending-control')
    }
    await wrapper.setProps({ ready: false })
    expect(wrapper.get('#repository-source').classes()).not.toContain('source-pending-control')
    expect(wrapper.get('.source-search button').attributes('disabled')).toBeDefined()
    expect(wrapper.get('.source-search button').classes()).not.toContain('source-pending-control')
    request.resolve(makeCatalog(2)); await flushPromises()
    expect(wrapper.get('.source-search button').attributes('disabled')).toBeDefined()
  })

  it('翻页失败保留原页和附件选择，恢复后可重试', async () => {
    const request = deferred<CatalogPage>()
    vi.mocked(downloadsApi.browse).mockResolvedValueOnce(makeCatalog(1, true)).mockReturnValueOnce(request.promise).mockResolvedValueOnce(makeCatalog(2))
    const wrapper = render(SourcePicker, { directory: 'F:\\下载', ready: true })
    await wrapper.get('#repository-source').setValue('test/repo'); await click(wrapper, '查找版本')
    await wrapper.get('.asset input').setValue(true)
    await wrapper.get('[aria-label="下一页版本"]').trigger('click')
    request.reject('GitHub 官方接口请求超时'); await flushPromises()
    expect(wrapper.get('[role="alert"]').text()).toContain('请求超时')
    expect(wrapper.get('.source-status').text()).toBe('')
    expect(wrapper.get('.source-browser').attributes('aria-busy')).toBe('false')
    expect(wrapper.get('.source-pagination').text()).toContain('第 1 页')
    expect(wrapper.text()).toContain('page-1.exe')
    expect((wrapper.get('.asset input').element as HTMLInputElement).checked).toBe(true)
    expect(wrapper.get('[aria-label="下一页版本"]').attributes('disabled')).toBeUndefined()
    await wrapper.get('[aria-label="下一页版本"]').trigger('click'); await flushPromises()
    expect(wrapper.find('[role="alert"]').exists()).toBe(false)
    expect(wrapper.get('.source-pagination').text()).toContain('第 2 页')
    expect(downloadsApi.browse).toHaveBeenCalledTimes(3)
  })

  it('过滤后空页仍能翻页，末页禁止继续请求', async () => {
    vi.mocked(downloadsApi.browse)
      .mockResolvedValueOnce(makeCatalog(1, true))
      .mockResolvedValueOnce({ ...makeCatalog(2, true), releases: [] })
      .mockResolvedValueOnce(makeCatalog(3))
    const wrapper = render(SourcePicker, { directory: 'F:\\下载', ready: true })
    await wrapper.get('#repository-source').setValue('test/repo'); await click(wrapper, '查找版本')
    await wrapper.get('[aria-label="下一页版本"]').trigger('click'); await flushPromises()
    expect(wrapper.text()).toContain('本页没有符合条件的版本')
    expect(wrapper.get('.source-pagination').text()).toContain('第 2 页')
    expect(wrapper.get('[aria-label="下一页版本"]').attributes('disabled')).toBeUndefined()
    await wrapper.get('[aria-label="下一页版本"]').trigger('click'); await flushPromises()
    expect(wrapper.text()).toContain('page-3.exe')
    expect(wrapper.get('[aria-label="下一页版本"]').attributes('disabled')).toBeDefined()
    await wrapper.get('[aria-label="下一页版本"]').trigger('click')
    expect(downloadsApi.browse).toHaveBeenCalledTimes(3)
  })

  it('查询失效后不接受旧分页结果，并清除加载状态', async () => {
    const request = deferred<CatalogPage>()
    vi.mocked(downloadsApi.browse).mockResolvedValueOnce(makeCatalog(1, true)).mockReturnValueOnce(request.promise)
    const wrapper = mount(SourcePicker, { props: { directory: 'F:\\下载', ready: true, prepareDirectory: async directory => directory }, attachTo: document.body }); wrappers.push(wrapper)
    await wrapper.get('#repository-source').setValue('test/repo'); await click(wrapper, '查找版本')
    await wrapper.get('[aria-label="下一页版本"]').trigger('click')
    await wrapper.setProps({ directory: 'F:\\新目录' })
    request.resolve(makeCatalog(2)); await flushPromises()
    expect(wrapper.get('.source-pagination').text()).toContain('第 1 页')
    expect(wrapper.text()).not.toContain('page-2.exe')
    expect(wrapper.get('.source-status').text()).toBe('')
    expect(wrapper.get('.source-browser').attributes('aria-busy')).toBe('false')
  })

  it.each(['成功', '失败'])('分页请求在卸载后%s不会影响新组件', async outcome => {
    const request = deferred<CatalogPage>()
    vi.mocked(downloadsApi.browse).mockReturnValueOnce(request.promise).mockResolvedValueOnce(makeCatalog(3))
    const wrapper = render(SourcePicker, { directory: 'F:\\下载', ready: true })
    await wrapper.get('#repository-source').setValue('test/repo'); await wrapper.get('.source-search').trigger('submit')
    wrapper.unmount(); wrappers.splice(wrappers.indexOf(wrapper), 1)
    const current = render(SourcePicker, { directory: 'F:\\下载', ready: true })
    await current.get('#repository-source').setValue('test/repo'); await click(current, '查找版本')
    if (outcome === '成功') request.resolve(makeCatalog(2))
    else request.reject('旧请求超时')
    await flushPromises()
    expect(current.text()).toContain('page-3.exe')
    expect(current.text()).not.toContain('page-2.exe')
    expect(current.find('[role="alert"]').exists()).toBe(false)
    expect(current.get('.source-status').text()).toBe('')
    expect(wrapper.emitted('snapshot')).toBeUndefined()
  })

  it('批量预览共用固定状态位和通用处理文案', async () => {
    const request = deferred<Awaited<ReturnType<typeof downloadsApi.previewBatch>>>()
    vi.mocked(downloadsApi.previewBatch).mockReturnValue(request.promise)
    const wrapper = render(SourcePicker, { directory: 'F:\\下载', ready: true })
    await wrapper.get('#project-source').setValue('https://github.com/test/repo/releases/download/v1/app.exe')
    const status = wrapper.get('.source-status').element
    await click(wrapper, '预览批量')
    expect(wrapper.get('.source-status').element).toBe(status)
    expect(status.textContent).toContain('正在处理，请稍候…')
    expect(wrapper.get('.source-batch button').attributes('disabled')).toBeDefined()
    expect(wrapper.get('.source-batch button').classes()).toContain('source-pending-control')
    request.reject('目录不可写'); await flushPromises()
    expect(status.textContent).toBe('')
    expect(wrapper.get('[role="alert"]').text()).toContain('目录不可写')
  })

  it('历史统计以当前页为范围，搜索和重置使用已有分页接口', async () => {
    const task = { ...makeTask('done', 'completed'), verification: 'failed' as const }
    vi.mocked(downloadsApi.history).mockResolvedValue({ items: [{ task, fileState: 'missing' }, { task: makeTask('failed', 'failed'), fileState: 'unfinished' }], total: 41, page: 1, pageSize: 20, revision: 1 })
    const wrapper = render(HistoryView, { ready: true }); await flushPromises()
    expect(wrapper.get('.history-stats').text()).toContain('匹配记录41')
    expect(wrapper.get('.history-stats').text()).toContain('本页已完成1')
    expect(wrapper.get('.history-stats').text()).toContain('本页下载失败1')
    expect(wrapper.text()).not.toContain('校验')
    expect(wrapper.findAll('.history-table thead th')).toHaveLength(8)
    expect(wrapper.findAll('.history-table tbody tr')[0]!.findAll('td')).toHaveLength(8)
    await wrapper.get('.history-search input').setValue('test/repo')
    await wrapper.get('.history-filters select').setValue('completed')
    await wrapper.get('.history-filters').trigger('submit'); await flushPromises()
    expect(downloadsApi.history).toHaveBeenLastCalledWith('test/repo', 'completed', 1)
    await click(wrapper, '重置')
    expect(downloadsApi.history).toHaveBeenLastCalledWith('', null, 1)
    await wrapper.get('#history-page').setValue('99'); await wrapper.get('.history-jump').trigger('submit')
    expect(wrapper.text()).toContain('请输入 1 至 3 之间的页码')
  })

  it('删除历史末页的最后一条记录会回到有效页', async () => {
    const task = makeTask('done', 'completed')
    vi.mocked(downloadsApi.history)
      .mockResolvedValueOnce({ items: [{ task, fileState: 'present' }], total: 21, page: 1, pageSize: 20, revision: 1 })
      .mockResolvedValueOnce({ items: [{ task, fileState: 'present' }], total: 21, page: 2, pageSize: 20, revision: 1 })
      .mockResolvedValueOnce({ items: [], total: 20, page: 2, pageSize: 20, revision: 2 })
      .mockResolvedValueOnce({ items: [{ task: makeTask('other', 'completed'), fileState: 'present' }], total: 20, page: 1, pageSize: 20, revision: 2 })
    vi.mocked(downloadsApi.act).mockResolvedValue(makeSnapshot())
    const wrapper = render(HistoryView, { ready: true }); await flushPromises()
    await wrapper.get('[aria-label="下一页"]').trigger('click'); await flushPromises()
    await click(wrapper, '移除记录')
    expect(downloadsApi.history).toHaveBeenLastCalledWith('', null, 1)
    expect(wrapper.text()).toContain('other.zip')
  })

  it('后端稍后就绪时历史页自动加载，卸载后忽略返回结果', async () => {
    const wrapper = mount(HistoryView, { props: { ready: false } }); wrappers.push(wrapper)
    expect(downloadsApi.history).not.toHaveBeenCalled()
    let finish!: (value: Awaited<ReturnType<typeof downloadsApi.history>>) => void
    vi.mocked(downloadsApi.history).mockReturnValue(new Promise(resolve => { finish = resolve }))
    await wrapper.setProps({ ready: true }); expect(downloadsApi.history).toHaveBeenCalledOnce()
    wrapper.unmount(); wrappers.splice(wrappers.indexOf(wrapper), 1)
    finish({ items: [], total: 0, page: 1, pageSize: 20, revision: 1 }); await flushPromises()
    expect(wrapper.emitted('snapshot')).toBeUndefined()
  })

  it('收藏筛选与列表切换不丢失来源，失败状态不会伪装成已是最新', async () => {
    const base: Favorite = { id: 'one', repository: 'test/one', latest: { id: 1, tag: 'v1', url: '' }, lastChecked: 2, lastSuccess: 1, nextCheck: null, error: null }
    const wrapper = render(FavoritesView, { ready: true, favorites: [base, { ...base, id: 'two', repository: 'test/two', error: '接口限流' }, { ...base, id: 'three', repository: 'test/three', latest: null, lastSuccess: null, lastChecked: null }] })
    await click(wrapper, '检查失败')
    expect(wrapper.findAll('.favorite-card')).toHaveLength(1); expect(wrapper.text()).toContain('test/two')
    await click(wrapper, '列表')
    expect(wrapper.get('.favorites-grid').classes()).toContain('favorites-grid--list')
    await click(wrapper, '查看版本')
    expect(wrapper.emitted('browse')?.[0]).toEqual(['test/two'])
    expect(wrapper.text()).not.toContain('已是最新')
  })

  it('恢复默认只修改草稿，保存后才调用服务', async () => {
    vi.mocked(downloadsApi.saveSettings).mockResolvedValue(makeSnapshot())
    const wrapper = render(SettingsView, { ready: true, settings: { limitKib: 512, closeToTray: true, autoCheck: true, backgroundCompletionNotice: true } })
    await click(wrapper, '恢复默认')
    expect(downloadsApi.saveSettings).not.toHaveBeenCalled()
    expect(wrapper.text()).toContain('保存设置后生效')
    await wrapper.get('form').trigger('submit'); await flushPromises()
    expect(downloadsApi.saveSettings).toHaveBeenCalledWith(defaultSettings())
    expect(wrapper.text()).toContain('设置已保存')
    await wrapper.get('#rate-limit').setValue('1024')
    expect(wrapper.text()).not.toContain('设置已保存')
  })

  it('不限速开关可恢复上次限速，非法值不会提交', async () => {
    const wrapper = render(SettingsView, { ready: true, settings: { ...defaultSettings(), limitKib: 5120 } })
    await wrapper.get('[role="switch"]').trigger('click')
    expect((wrapper.get('#rate-limit').element as HTMLInputElement).value).toBe('0')
    await wrapper.get('[role="switch"]').trigger('click')
    expect((wrapper.get('#rate-limit').element as HTMLInputElement).value).toBe('5.24288')
    await wrapper.get('#rate-limit').setValue('0.0001'); await wrapper.get('form').trigger('submit')
    expect(downloadsApi.saveSettings).not.toHaveBeenCalled()
    expect(wrapper.text()).toContain('请输入 0（不限速）或 0.001024 至 10240 MB/s 之间的数值')
  })

  it('线路表不将未检测显示为可用，空输入及活动任务仍可手动检测', async () => {
    const wrapper = mount(RouteDiagnostics, { props: { ready: true, diagnosing: false, hasActive: true, url: '', reports: [] } }); wrappers.push(wrapper)
    expect(wrapper.text()).toContain('尚未检测')
    expect(wrapper.get('button').attributes('disabled')).toBeUndefined()
    expect(wrapper.text()).toContain('下载继续进行')
    expect(wrapper.text()).toContain('默认测试文件')
    await click(wrapper, '检测线路')
    expect(wrapper.emitted('diagnose')).toHaveLength(1)
    await wrapper.setProps({ diagnosing: true })
    expect(wrapper.get('button').attributes('disabled')).toBeDefined()
    expect(wrapper.text()).toContain('请等待本轮检测结束')
    await wrapper.setProps({ diagnosing: false, ready: false })
    expect(wrapper.get('button').attributes('disabled')).toBeDefined()
    expect(wrapper.text()).toContain('任务管理器尚未就绪')
  })
})
