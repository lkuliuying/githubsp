import { flushPromises, mount } from '@vue/test-utils'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import DownloadNotice from './components/DownloadNotice.vue'
import TaskRow from './components/TaskRow.vue'
import { formatElapsed } from './format'
import { noticeApi } from './services/notices'
import type { DownloadNoticeView, DownloadTask } from './types'

vi.mock('./services/notices', () => ({ noticeApi: { read: vi.fn(), presented: vi.fn(), dismiss: vi.fn(), hover: vi.fn(), open: vi.fn(), subscribe: vi.fn() } }))
const wrappers: ReturnType<typeof mount>[] = []
const state = (revision = 1, filename = '中文文件.zip'): DownloadNoticeView => ({ revision, notice: { count: 1, filename, elapsedMs: 138000, elapsedIsPartial: false } })
let update: (value: DownloadNoticeView) => void
let unlisten = vi.fn()
function deferred<T>() {
  let resolve!: (value: T) => void
  const promise = new Promise<T>(done => { resolve = done })
  return { promise, resolve }
}
function render() { const wrapper = mount(DownloadNotice, { attachTo: document.body }); wrappers.push(wrapper); return wrapper }
beforeEach(() => {
  vi.resetAllMocks(); unlisten = vi.fn()
  vi.mocked(noticeApi.subscribe).mockImplementation(async callback => { update = callback; return unlisten })
  vi.mocked(noticeApi.read).mockResolvedValue(state())
  vi.mocked(noticeApi.presented).mockResolvedValue()
  vi.mocked(noticeApi.hover).mockResolvedValue()
})
afterEach(() => { wrappers.splice(0).forEach(wrapper => wrapper.unmount()); document.body.innerHTML = '' })

describe('累计耗时格式', () => {
  it.each([
    [undefined, '未知'], [null, '未知'], [NaN, '未知'], [Infinity, '未知'], [-1, '未知'],
    [0, '不足 1 秒'], [1, '不足 1 秒'], [999, '不足 1 秒'], [1000, '1 秒'], [35000, '35 秒'],
    [59999, '59 秒'], [60000, '1 分 0 秒'], [138000, '2 分 18 秒'], [3599999, '59 分 59 秒'],
    [3789000, '1 小时 03 分 09 秒'], [90061000, '25 小时 01 分 01 秒'],
  ])('%s 毫秒显示为 %s', (value, expected) => { expect(formatElapsed(value)).toBe(expected) })
})

describe('独立桌面提醒', () => {
  it('先订阅再读取，渲染完成后确认显示并传递交互状态', async () => {
    const wrapper = render(); await flushPromises()
    expect(vi.mocked(noticeApi.subscribe).mock.invocationCallOrder[0]).toBeLessThan(vi.mocked(noticeApi.read).mock.invocationCallOrder[0]!)
    expect(wrapper.text()).toContain('中文文件.zip')
    expect(wrapper.text()).toContain('2 分 18 秒')
    expect(noticeApi.presented).toHaveBeenCalledWith(1)
    await wrapper.get('main').trigger('mouseenter'); await flushPromises()
    expect(noticeApi.hover).toHaveBeenLastCalledWith(1, true)
    await wrapper.get('main').trigger('mouseleave'); await flushPromises()
    expect(noticeApi.hover).toHaveBeenLastCalledWith(1, false)
  })

  it('新事件先到时忽略落后的初始读取，合并消息使用最新修订', async () => {
    const initial = deferred<DownloadNoticeView>()
    vi.mocked(noticeApi.read).mockReturnValue(initial.promise)
    const wrapper = render(); await flushPromises()
    const next = state(2, '新文件.zip'); next.notice!.count = 3; next.notice!.elapsedIsPartial = true
    update(next); await flushPromises()
    initial.resolve(state(1, '旧文件.zip')); await flushPromises()
    expect(wrapper.text()).toContain('已完成 3 项下载')
    expect(wrapper.text()).toContain('记录不完整')
    expect(wrapper.text()).not.toContain('旧文件.zip')
    await wrapper.get('[aria-label="关闭提醒"]').trigger('click'); await flushPromises()
    expect(noticeApi.dismiss).toHaveBeenCalledWith(2)
  })

  it('查看下载失败时可重试，成功只调用通知专用接口', async () => {
    vi.mocked(noticeApi.open).mockRejectedValueOnce('主窗口不可用').mockResolvedValueOnce()
    const wrapper = render(); await flushPromises()
    await wrapper.get('.pa-btn--primary').trigger('click'); await flushPromises()
    expect(wrapper.get('[role="alert"]').text()).toBe('主窗口不可用')
    await wrapper.get('.pa-btn--primary').trigger('click'); await flushPromises()
    expect(noticeApi.open).toHaveBeenCalledTimes(2)
    expect(wrapper.find('[role="alert"]').exists()).toBe(false)
  })

  it('关闭消息后清空内容，卸载后不再处理事件或滞后的订阅', async () => {
    const wrapper = render(); await flushPromises()
    update({ revision: 2, notice: null }); await flushPromises()
    expect(wrapper.text()).not.toContain('下载完成')
    wrapper.unmount(); wrappers.splice(wrappers.indexOf(wrapper), 1)
    expect(unlisten).toHaveBeenCalledOnce()
    const before = vi.mocked(noticeApi.presented).mock.calls.length
    update(state(3)); await flushPromises()
    expect(noticeApi.presented).toHaveBeenCalledTimes(before)
    const subscription = deferred<() => void>()
    vi.mocked(noticeApi.subscribe).mockReturnValue(subscription.promise)
    const late = render(); late.unmount(); wrappers.splice(wrappers.indexOf(late), 1)
    const stop = vi.fn(); subscription.resolve(stop); await flushPromises()
    expect(stop).toHaveBeenCalledOnce()
  })
})

it('任务详情展示实时累计值、终态总耗时和旧记录未知状态', async () => {
  const task: DownloadTask = { id: 'one', url: 'https://github.com/test/repo/releases/download/v1/file.zip', filename: 'file.zip', directory: 'F:\\下载', status: 'paused', downloaded: 1, total: 2, speed: 0, eta: null, route: null, verification: 'pending', error: null, finalPath: null, createdAt: 1, revision: 1 }
  const wrapper = mount(TaskRow, { props: { task, busy: false }, global: { stubs: { RouteControls: true } } }); wrappers.push(wrapper)
  await wrapper.get('[aria-label="下载详情：file.zip"]').trigger('click')
  expect(wrapper.text()).toContain('已耗时未知')
  await wrapper.setProps({ task: { ...task, status: 'downloading', elapsedMs: 35000 } })
  expect(wrapper.text()).toContain('已耗时35 秒')
  await wrapper.setProps({ task: { ...task, status: 'completed', elapsedMs: 138000, elapsedIsPartial: true } })
  expect(wrapper.text()).toContain('总耗时2 分 18 秒（记录不完整）')
})
