import { flushPromises, mount } from '@vue/test-utils'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import TaskRow from './components/TaskRow.vue'
import SettingsView from './components/SettingsView.vue'
import RouteDiagnostics from './components/RouteDiagnostics.vue'
import { formatBytes, taskSpeed, taskFailure } from './format'
import { downloadsApi } from './services/downloads'
import { defaultSettings, type DownloadTask, type Settings, type Snapshot } from './types'

vi.mock('./services/downloads', () => ({ downloadsApi: { saveSettings: vi.fn() }, errorMessage: (value: unknown) => String(value) }))
const wrappers: ReturnType<typeof mount>[] = []
const task = (overrides: Partial<DownloadTask> = {}): DownloadTask => ({
  id: 'one', url: 'https://github.com/test/repo/releases/download/v1/file.zip', filename: '中文下载文件.zip',
  directory: 'F:\\下载', status: 'completed', downloaded: 100000000, total: 100000000, speed: 0, eta: null,
  route: 'GitHub 直连', verification: 'verified', error: null, finalPath: null, createdAt: 1, revision: 1,
  elapsedMs: 20000, elapsedIsPartial: false, ...overrides,
})
const snapshot = (settings: Settings): Snapshot => ({ tasks: [], lastDirectory: null, error: null, revision: 1, settings, queueRevision: 0, diagnostics: [], diagnosing: false, notices: [], favorites: [] })
function settingsView(settings: Settings = defaultSettings()) {
  const wrapper = mount(SettingsView, { props: { settings, ready: true } }); wrappers.push(wrapper); return wrapper
}
function taskView(value: DownloadTask) {
  const wrapper = mount(TaskRow, { props: { task: value, busy: false } }); wrappers.push(wrapper); return wrapper
}
beforeEach(() => {
  vi.resetAllMocks()
  vi.mocked(downloadsApi.saveSettings).mockImplementation(async settings => snapshot(settings))
})
afterEach(() => wrappers.splice(0).forEach(wrapper => wrapper.unmount()))

describe('十进制容量与速度', () => {
  it('检测来源与真实文件名独立显示，历史结果不推断来源，检测时隐藏旧速度', async () => {
    const reports = [{ id: 'github', name: 'GitHub 直连', available: true, bytesPerSecond: 5000000, checkedAt: 1, error: null }]
    const wrapper = mount(RouteDiagnostics, { props: { reports, diagnosing: false, ready: true, hasActive: false, url: '' } }); wrappers.push(wrapper)
    expect(wrapper.text()).toContain('历史检测结果')
    expect(wrapper.text()).not.toContain('指定附件：')
    await wrapper.setProps({ context: { source: 'input', filename: 'DLSS5-Swapper-Setup-2.2.9.exe' }, diagnosing: true })
    expect(wrapper.get('.route-target').text()).toContain('指定附件：DLSS5-Swapper-Setup-2.2.9.exe')
    expect(wrapper.text()).not.toContain('5.0 MB/s')
    expect(wrapper.text()).not.toContain('最近检测最快')
    await wrapper.setProps({ context: { source: 'download', filename: 'v2rayN-windows-64-desktop.zip' }, diagnosing: false })
    expect(wrapper.get('.route-target').text()).toContain('下载自动检测：v2rayN-windows-64-desktop.zip')
    await wrapper.setProps({ reports: [{ ...reports[0]!, available: false, bytesPerSecond: 0, error: '线路探测超时' }] })
    expect(wrapper.text()).toContain('本轮线路检测失败')
    expect(wrapper.text()).toContain('线路探测超时')
    expect(wrapper.get('button').attributes('disabled')).toBeUndefined()
  })

  it.each([
    [null, '未知大小'], [NaN, '未知大小'], [Infinity, '未知大小'], [-1, '未知大小'],
    [0, '0 B'], [999, '999 B'], [1000, '1.0 KB'], [1024, '1.0 KB'],
    [999999, '1.0 MB'], [1000000, '1.0 MB'], [5000000, '5.0 MB'],
    [104857600, '105 MB'], [1000000000, '1.0 GB'], [1000000000000, '1.0 TB'],
  ])('%s 字节显示为 %s', (value, expected) => expect(formatBytes(value)).toBe(expected))

  it('线路测速与探测大小按真实字节数换算', () => {
    const wrapper = mount(RouteDiagnostics, { props: { reports: [{ id: 'github', name: 'GitHub 直连', available: true, bytesPerSecond: 5000000, checkedAt: 1, error: null }], diagnosing: false, ready: true, hasActive: false, url: task().url } }); wrappers.push(wrapper)
    expect(wrapper.text()).toContain('5.0 MB/s')
    expect(wrapper.text()).toContain('524 KB')
    expect(wrapper.text()).not.toMatch(/KiB|MiB/)
  })
})

describe('任务速度展示', () => {
  it('传输与暂停恢复使用实时速度，完成后按毫秒耗时计算并忽略最后实时速度', async () => {
    const value = task({ status: 'downloading', speed: 7000000 })
    const wrapper = taskView(value)
    expect(wrapper.get('.task-speed').text()).toBe('7.0 MB/s')
    await wrapper.setProps({ task: { ...value, status: 'paused' } })
    expect(wrapper.get('.task-speed').text()).toBe('—')
    await wrapper.setProps({ task: { ...value, speed: 2000000, elapsedMs: 40000 } })
    expect(wrapper.get('.task-speed').text()).toBe('2.0 MB/s')
    await wrapper.setProps({ task: task() })
    expect(wrapper.get('.task-speed').text()).toBe('平均 5.0 MB/s')
    await wrapper.setProps({ task: task({ speed: 9000000 }) })
    expect(wrapper.get('.task-speed').text()).toBe('平均 5.0 MB/s')
  })

  it.each(['queued', 'probing', 'retrying', 'verifying', 'pausing', 'cancelling', 'paused', 'failed', 'cancelled'] as const)('%s 不显示上次速度或平均值', status => {
    expect(taskSpeed(task({ status, speed: 5000000 })).text).toBe('—')
  })

  it.each([undefined, null, 0, -1, NaN, Infinity])('无效耗时 %s 不产生伪造平均值', elapsedMs => {
    const speed = taskSpeed(task({ elapsedMs }))
    expect(speed.text).toBe('—'); expect(speed.title).toContain('缺少有效耗时')
  })

  it('不完整耗时、无效大小及无效实时速度有明确占位', () => {
    expect(taskSpeed(task({ elapsedIsPartial: true })).title).toContain('记录不完整')
    expect(taskSpeed(task({ total: NaN })).text).toBe('—')
    expect(taskSpeed(task({ total: -1 })).text).toBe('—')
    expect(taskSpeed(task({ status: 'downloading', speed: Infinity })).text).toBe('—')
    const wrapper = taskView(task({ elapsedIsPartial: true }))
    expect(wrapper.get('.task-speed').attributes('aria-label')).toContain('无法计算平均速度')
  })

  it('不足一秒使用实际毫秒，空文件和缺少 total 的完成记录可计算', () => {
    expect(taskSpeed(task({ total: 1000000, elapsedMs: 200 })).text).toBe('平均 5.0 MB/s')
    expect(taskSpeed(task({ total: 0, downloaded: 0, elapsedMs: 200 })).text).toBe('平均 0 B/s')
    expect(taskSpeed(task({ total: null, downloaded: 420, elapsedMs: 200 })).text).toBe('平均 2.1 KB/s')
  })
})

describe('隐藏校验展示', () => {
  it.each(['verified', 'unverified', 'pending'] as const)('%s 不展示常态校验信息，详情跨度与列表列数一致', async verification => {
    const wrapper = taskView(task({ verification }))
    expect(wrapper.text()).not.toMatch(/校验|SHA-256|摘要/)
    expect(wrapper.findAll('tr')[0]!.findAll('td')).toHaveLength(9)
    await wrapper.get('[aria-expanded]').trigger('click')
    expect(wrapper.get('.task-detail-row > td').attributes('colspan')).toBe('9')
  })

  it('收尾显示正在完成下载，完整性失败仍提供原因与操作且不修改内部记录', async () => {
    const wrapper = taskView(task({ status: 'verifying', verification: 'pending' }))
    expect(wrapper.text()).toContain('正在完成下载')
    const value = task({ status: 'failed', verification: 'failed', error: 'SHA-256 与 GitHub 官方摘要不一致', failure: { category: 'integrity', message: '原始详细错误', action: '校验失败，切换线路重新下载' } })
    await wrapper.setProps({ task: value })
    await wrapper.get('[aria-expanded]').trigger('click')
    expect(wrapper.text()).toContain('下载文件内容异常')
    expect(wrapper.text()).toContain('请切换线路重新下载')
    expect(wrapper.text()).not.toMatch(/校验|SHA-256|摘要/)
    await wrapper.get('[aria-label="重试下载"]').trigger('click')
    expect(wrapper.emitted('action')?.[0]).toEqual(['one', 'resume'])
    expect(value.error).toContain('SHA-256')
    expect(value.failure?.message).toBe('原始详细错误')
    expect(taskFailure(task({ error: '下载服务返回 HTTP 404' })).message).toBe('下载服务返回 HTTP 404')
  })
})

describe('MB/s 限速输入', () => {
  it.each([1, 512, 1024, 5120, 9999999, 10000000])('原有 %s KiB/s 在保存其他设置时精确保留', async limitKib => {
    const wrapper = settingsView({ ...defaultSettings(), limitKib })
    expect(Number((wrapper.get('#rate-limit').element as HTMLInputElement).value)).toBe(Number((limitKib * 1024 / 1000000).toFixed(6)))
    await wrapper.findAll('input[type="checkbox"]')[0]!.setValue(true)
    await wrapper.get('form').trigger('submit'); await flushPromises()
    expect(downloadsApi.saveSettings).toHaveBeenCalledWith({ ...defaultSettings(), limitKib, closeToTray: true })
  })

  it.each([['5', 4883, '5.000192'], ['1.5', 1465, '1.50016'], ['0.001024', 1, '0.001024'], ['10240', 10000000, '10240'], ['0', 0, '0']] as const)('输入 %s MB/s 转换为 %s 并回显实际值', async (input, limitKib, actual) => {
    const wrapper = settingsView()
    await wrapper.get('#rate-limit').setValue(input)
    await wrapper.get('form').trigger('submit'); await flushPromises()
    expect(downloadsApi.saveSettings).toHaveBeenCalledWith({ ...defaultSettings(), limitKib })
    expect((wrapper.get('#rate-limit').element as HTMLInputElement).value).toBe(actual)
    expect(wrapper.text()).toContain('设置已保存')
    await wrapper.setProps({ settings: { ...defaultSettings(), limitKib } }); await flushPromises()
    expect(wrapper.text()).toContain('设置已保存')
    expect(wrapper.text()).not.toContain('有尚未保存的设置')
  })

  it.each(['', '-1', '0.000001', '0.001023', '10240.000001', 'Infinity', 'NaN', 'abc'])('拒绝无效输入 %s，正数不会意外变成不限速', async input => {
    const wrapper = settingsView()
    await wrapper.get('#rate-limit').setValue(input)
    await wrapper.get('form').trigger('submit'); await flushPromises()
    expect(downloadsApi.saveSettings).not.toHaveBeenCalled()
    expect(wrapper.get('[role="alert"]').text()).toContain('0.001024 至 10240 MB/s')
  })

  it('滑块使用十进制范围和步长，切换不限速保留草稿，失败后仍可修改重试', async () => {
    const wrapper = settingsView()
    const range = wrapper.get('#rate-range')
    expect(range.attributes('max')).toBe('100'); expect(range.attributes('step')).toBe('0.25')
    await range.setValue('5.25')
    await wrapper.get('[role="switch"]').trigger('click')
    expect((wrapper.get('#rate-limit').element as HTMLInputElement).value).toBe('0')
    await wrapper.get('[role="switch"]').trigger('click')
    expect((wrapper.get('#rate-limit').element as HTMLInputElement).value).toBe('5.25')
    await wrapper.setProps({ settings: { ...defaultSettings() } })
    expect((wrapper.get('#rate-limit').element as HTMLInputElement).value).toBe('5.25')
    vi.mocked(downloadsApi.saveSettings).mockRejectedValueOnce('保存失败')
    await wrapper.get('form').trigger('submit'); await flushPromises()
    expect(wrapper.get('[role="alert"]').text()).toBe('保存失败')
    expect((wrapper.get('#rate-limit').element as HTMLInputElement).value).toBe('5.25')
    await wrapper.get('form').trigger('submit'); await flushPromises()
    expect(downloadsApi.saveSettings).toHaveBeenLastCalledWith({ ...defaultSettings(), limitKib: 5127 })
    expect((wrapper.get('#rate-limit').element as HTMLInputElement).value).toBe('5.250048')
  })
})
