import { mount, flushPromises } from '@vue/test-utils'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import SourcePicker from './components/SourcePicker.vue'
import SettingsView from './components/SettingsView.vue'
import HistoryView from './components/HistoryView.vue'
import FavoritesView from './components/FavoritesView.vue'
import UpdateView from './components/UpdateView.vue'
import { downloadsApi } from './services/downloads'
import type { CatalogPage, Snapshot, PrepareDirectory } from './types'

vi.mock('./services/downloads', () => ({ downloadsApi: { browse: vi.fn(), previewBatch: vi.fn(), createBatch: vi.fn(), saveSettings: vi.fn(), history: vi.fn(), act: vi.fn(), copy: vi.fn(), openDirectory: vi.fn(), favorite: vi.fn(), checkFavorites: vi.fn(), removeFavorite: vi.fn(), checkUpdate: vi.fn(), openRelease: vi.fn() }, errorMessage: (value: unknown) => String(value) }))
const snapshot: Snapshot = { tasks: [], lastDirectory: 'F:\\下载', error: null, revision: 1, queueRevision: 0, settings: { limitKib: 0, closeToTray: false, autoCheck: false, backgroundCompletionNotice: true }, diagnostics: [], diagnosing: false, notices: [], favorites: [] }
const url = 'https://github.com/test/repo/releases/download/v1/file.exe'
const catalog: CatalogPage = { repository: 'test/repo', page: 0, hasMore: false, selectedUrl: null, releases: [{ id: 1, tag: 'v1', name: 'v1', prerelease: false, notes: '', url: 'https://github.com/test/repo/releases/tag/v1', assets: [{ id: 2, name: '中文安装包-x64.exe', url, size: 1024, sha256: 'a'.repeat(64), hints: ['Windows', 'x64'] }] }] }
const wrappers: ReturnType<typeof mount>[] = []
const prepareDirectory = vi.fn<PrepareDirectory>()
function render(component: Parameters<typeof mount>[0], props: Record<string, unknown>) { const wrapper = mount(component, { props: { ...(component === SourcePicker ? { prepareDirectory } : {}), ...props }, attachTo: document.body }); wrappers.push(wrapper); return wrapper }
const click = async (wrapper: ReturnType<typeof mount>, label: string) => { const button = wrapper.findAll('button').find(button => button.text().includes(label)); expect(button, label).toBeDefined(); await button!.trigger('click'); await flushPromises() }
beforeEach(() => { vi.resetAllMocks(); prepareDirectory.mockImplementation(async directory => directory.trim()) })
afterEach(() => { wrappers.splice(0).forEach(wrapper => wrapper.unmount()); document.body.innerHTML = '' })

describe('三批功能用户流程', () => {
  it('仓库附件多选先预览，部分失败保留可修改链接', async () => {
    vi.mocked(downloadsApi.browse).mockResolvedValue(catalog)
    vi.mocked(downloadsApi.previewBatch).mockResolvedValue({ items: [{ input: url, url, filename: '中文安装包-x64.exe', size: 1024, status: 'valid', message: null, taskId: null }], knownSize: 1024, unknownCount: 0, preflight: { directory: 'F:\\下载', available: 10000, required: 2048, warning: null } })
    vi.mocked(downloadsApi.createBatch).mockResolvedValue({ items: [{ input: url, url: null, filename: null, size: null, status: 'failed', message: '目录不可写', taskId: null }], snapshot })
    const wrapper = render(SourcePicker, { directory: 'F:\\下载', ready: true })
    await wrapper.get('#project-source').setValue('https://github.com/test/repo'); await click(wrapper, '查找版本')
    expect(wrapper.text()).toContain('Windows · x64')
    expect(wrapper.get('.asset').text()).toContain('1.0 KB')
    expect(wrapper.get('.asset').text()).not.toMatch(/SHA-256|摘要/)
    await wrapper.get('.asset input').setValue(true); await click(wrapper, '预览所选')
    expect(wrapper.text()).toContain('已知总大小 1.0 KB')
    expect(wrapper.text()).toContain('可用空间 10.0 KB')
    expect(downloadsApi.createBatch).not.toHaveBeenCalled(); expect(wrapper.text()).toContain('有效 1 项')
    await click(wrapper, '确认创建'); expect(downloadsApi.createBatch).toHaveBeenCalledWith([url], 'F:\\下载', null)
    expect((wrapper.get('#project-source').element as HTMLTextAreaElement).value).toBe(url); expect(wrapper.text()).toContain('目录不可写')
  })
  it('保存目录变化使正在返回的批量预览失效', async () => {
    let finish!: (value: Awaited<ReturnType<typeof downloadsApi.previewBatch>>) => void
    vi.mocked(downloadsApi.previewBatch).mockReturnValue(new Promise(resolve => { finish = resolve }))
    const wrapper = mount(SourcePicker, { props: { directory: 'F:\\旧目录', ready: true, prepareDirectory } }); wrappers.push(wrapper); await wrapper.get('#project-source').setValue(url); await click(wrapper, '预览批量')
    await wrapper.setProps({ directory: 'F:\\新目录' }); finish({ items: [], knownSize: 0, unknownCount: 0, preflight: { directory: 'F:\\旧目录', available: 100, required: 0, warning: null } }); await flushPromises()
    expect(wrapper.find('[aria-label="批量预览"]').exists()).toBe(false)
  })
  it('目录准备取消时不请求批量预览，保留原链接', async () => {
    prepareDirectory.mockResolvedValue(null)
    const wrapper = render(SourcePicker, { directory: 'F:\\下载', ready: true })
    await wrapper.get('#project-source').setValue(url); await click(wrapper, '预览批量')
    expect(downloadsApi.previewBatch).not.toHaveBeenCalled()
    expect(downloadsApi.createBatch).not.toHaveBeenCalled()
    expect((wrapper.get('#project-source').element as HTMLTextAreaElement).value).toBe(url)
  })
  it('等待目录准备时修改目录使旧批量操作失效', async () => {
    let finish!: (directory: string) => void
    prepareDirectory.mockReturnValue(new Promise(resolve => { finish = resolve }))
    const wrapper = mount(SourcePicker, { props: { directory: 'F:\\旧目录', ready: true, prepareDirectory } })
    wrappers.push(wrapper)
    await wrapper.get('#project-source').setValue(url); await click(wrapper, '预览批量')
    await wrapper.setProps({ directory: 'F:\\新目录' })
    finish('F:\\旧目录'); await flushPromises()
    expect(downloadsApi.previewBatch).not.toHaveBeenCalled()
    expect(downloadsApi.createBatch).not.toHaveBeenCalled()
  })
  it('限速拒绝非法值，保存完整设置并显示成功', async () => {
    vi.mocked(downloadsApi.saveSettings).mockResolvedValue({ ...snapshot, settings: { ...snapshot.settings, limitKib: 5000, closeToTray: true } })
    const wrapper = render(SettingsView, { settings: snapshot.settings, ready: true })
    await wrapper.get('#rate-limit').setValue('-1'); await wrapper.get('form').trigger('submit'); expect(downloadsApi.saveSettings).not.toHaveBeenCalled()
    await wrapper.get('#rate-limit').setValue('5.12'); await wrapper.findAll('input[type="checkbox"]')[0]!.setValue(true); await wrapper.get('form').trigger('submit'); await flushPromises()
    expect(downloadsApi.saveSettings).toHaveBeenCalledWith({ limitKib: 5000, closeToTray: true, autoCheck: false, backgroundCompletionNotice: true }); expect(wrapper.text()).toContain('设置已保存')
  })
  it('下载进度快照不会覆盖尚未保存的限速输入', async () => {
    const wrapper = mount(SettingsView, { props: { settings: { ...snapshot.settings }, ready: true } }); wrappers.push(wrapper)
    await wrapper.get('#rate-limit').setValue('512')
    await wrapper.setProps({ settings: { ...snapshot.settings } })
    expect((wrapper.get('#rate-limit').element as HTMLInputElement).value).toBe('512')
  })
  it('后台完成提醒默认开启，关闭后保存完整设置', async () => {
    vi.mocked(downloadsApi.saveSettings).mockResolvedValue({ ...snapshot, settings: { ...snapshot.settings, backgroundCompletionNotice: false } })
    const wrapper = render(SettingsView, { settings: snapshot.settings, ready: true })
    const option = wrapper.findAll('label').find(label => label.text().includes('后台下载完成提醒'))!
    expect((option.get('input').element as HTMLInputElement).checked).toBe(true)
    await option.get('input').setValue(false)
    await wrapper.get('form').trigger('submit'); await flushPromises()
    expect(downloadsApi.saveSettings).toHaveBeenCalledWith({ ...snapshot.settings, backgroundCompletionNotice: false })
  })
  it('历史文件缺失显示位置和未知完成时间，移除只调用记录操作', async () => {
    const task = { id: 'x', url, filename: 'file.exe', directory: 'F:\\下载', status: 'completed' as const, downloaded: 1, total: 1, speed: 0, eta: null, route: null, verification: 'unverified' as const, error: null, finalPath: 'F:\\下载\\file.exe', createdAt: 1, revision: 1 }
    vi.mocked(downloadsApi.history).mockResolvedValue({ items: [{ task, fileState: 'missing' }], total: 1, page: 1, pageSize: 20, revision: 1 }); vi.mocked(downloadsApi.act).mockResolvedValue(snapshot)
    const wrapper = render(HistoryView, { ready: true }); await flushPromises(); expect(wrapper.text()).toContain('文件已不在原位置'); expect(wrapper.text()).toContain('完成：未知'); await click(wrapper, '移除记录'); expect(downloadsApi.act).toHaveBeenCalledWith('x', 'remove')
  })
  it('收藏检查失败仍展示上次成功版本，并能返回附件选择器', async () => {
    const favorite = { id: 'f', repository: 'test/repo', latest: { id: 1, tag: 'v1', url: 'page' }, lastChecked: 100, lastSuccess: 50, nextCheck: 200, error: '接口限流' }
    const wrapper = render(FavoritesView, { ready: true, favorites: [favorite] }); expect(wrapper.text()).toContain('最新正式版：v1'); expect(wrapper.text()).toContain('接口限流'); await click(wrapper, '查看版本'); expect(wrapper.emitted('browse')?.[0]).toEqual(['test/repo'])
  })
  it('更新源未配置不伪报最新版，版本说明作为文本显示', async () => {
    vi.mocked(downloadsApi.checkUpdate).mockResolvedValue({ status: 'not_configured', current: '0.2.0', latest: null, notes: null, url: null, message: '尚未配置官方发布源', nextCheck: null })
    const wrapper = render(UpdateView, { ready: true }); await click(wrapper, '检查软件更新'); expect(wrapper.text()).toContain('尚未配置官方发布源'); expect(wrapper.text()).not.toContain('已是最新版'); expect(downloadsApi.openRelease).not.toHaveBeenCalled()
    vi.mocked(downloadsApi.checkUpdate).mockResolvedValue({ status: 'available', current: '0.2.0', latest: 'v0.3.0', notes: '<script>unsafe</script>', url: 'https://github.com/test/repo/releases/tag/v0.3.0', message: '有新版本', nextCheck: null }); await click(wrapper, '检查软件更新'); expect(wrapper.find('script').exists()).toBe(false); expect(wrapper.get('pre').text()).toContain('<script>'); await click(wrapper, '打开官方'); expect(downloadsApi.openRelease).toHaveBeenCalledWith('https://github.com/test/repo/releases/tag/v0.3.0')
  })
})
