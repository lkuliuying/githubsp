import { mount, flushPromises } from '@vue/test-utils'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import UpdateNotes from './components/UpdateNotes.vue'
import UpdateView from './components/UpdateView.vue'
import { downloadsApi } from './services/downloads'
import { normalizeUpdateLink } from './updateNotes'

vi.mock('./services/downloads', () => ({
  downloadsApi: { checkUpdate: vi.fn(), openUpdateLink: vi.fn(), openRelease: vi.fn() },
  errorMessage: (value: unknown) => value instanceof Error ? value.message : String(value),
}))

const comparison = 'https://github.com/lkuliuying/githubsp/compare/v0.2.1...v0.2.2'
const notes = `## 验证

- **前端测试**、*Rust 测试*通过
  - 支持嵌套项目
- 检查 \`SHA-256\`

1. 退出旧版
2. 启动新版

> 由用户确认后下载。

| 平台 | 架构 |
| --- | --- |
| Windows | x64 |

\`\`\`powershell
if ($ready) {
    Write-Output '<测试>'
}
\`\`\`

[**查看完整变更**](${comparison})`
const result = { status: 'current', current: '0.2.2', latest: 'v0.2.2', notes, url: 'https://github.com/lkuliuying/githubsp/releases/tag/v0.2.2', message: '当前版本无需更新', nextCheck: null }
const wrappers: ReturnType<typeof mount>[] = []
function render(source = notes, ready = true) {
  const wrapper = mount(UpdateNotes, { props: { notes: source, ready }, attachTo: document.body })
  wrappers.push(wrapper)
  return wrapper
}
async function renderView() {
  const wrapper = mount(UpdateView, { props: { ready: true }, attachTo: document.body })
  wrappers.push(wrapper)
  await wrapper.get('.update-check').trigger('click')
  await flushPromises()
  return wrapper
}
beforeEach(() => { vi.resetAllMocks(); vi.mocked(downloadsApi.checkUpdate).mockResolvedValue(result) })
afterEach(() => { wrappers.splice(0).forEach(wrapper => wrapper.unmount()); document.body.innerHTML = '' })

describe('更新说明 Markdown 阅读', () => {
  it('渲染标题、列表、强调、代码、引用和表格，并保留代码缩进', () => {
    const wrapper = render()
    expect(wrapper.get('h2').text()).toBe('验证')
    expect(wrapper.get('li strong').text()).toBe('前端测试')
    expect(wrapper.get('li em').text()).toBe('Rust 测试')
    expect(wrapper.get('ul ul li').text()).toBe('支持嵌套项目')
    expect(wrapper.findAll('ol > li')).toHaveLength(2)
    expect(wrapper.get('li code').text()).toBe('SHA-256')
    expect(wrapper.get('blockquote').text()).toBe('由用户确认后下载。')
    expect(wrapper.get('tbody tr').text()).toContain('Windows')
    expect(wrapper.get('pre code').element.textContent).toContain("\n    Write-Output '<测试>'\n")
    expect(wrapper.get('[role="link"]').text()).toBe('查看完整变更')
    expect(wrapper.find('a').exists()).toBe(false)
  })

  it('原始 HTML 与图片只显示文本，不创建脚本、图片或可导航元素', () => {
    const wrapper = render('<script>alert(1)</script>\n\n<img src="https://example.com/a" onerror="alert(1)">\n\n<iframe src="https://example.com"></iframe>\n\n![图片说明](https://example.com/image.png)\n\n![**加粗描述**](data:image/svg+xml,test)')
    expect(wrapper.find('script, img, iframe, svg, a').exists()).toBe(false)
    expect(wrapper.text()).toContain('<script>alert(1)</script>')
    expect(wrapper.text()).toContain('图片说明')
    expect(wrapper.find('[onerror], [onclick]').exists()).toBe(false)
    expect(downloadsApi.openUpdateLink).not.toHaveBeenCalled()
  })

  it('图片替代文字中的标记不会成为 HTML', () => {
    const wrapper = render('![<img src=x onerror=alert(1)>](https://example.com/pic.png)')
    expect(wrapper.find('img').exists()).toBe(false)
    expect(wrapper.text()).toContain('<img src=x onerror=alert(1)>')
  })

  it('站外及相对链接保留标签文本，禁止执行危险协议', () => {
    const wrapper = render('[文档](https://example.com/docs) [相对路径](../compare/v1...v2) [脚本](javascript:alert%281%29) [文件](file:///C:/test.exe)')
    expect(wrapper.text()).toContain('文档')
    expect(wrapper.text()).toContain('相对路径')
    expect(wrapper.find('[role="link"], a, button').exists()).toBe(false)
  })

  it('空白内容可以渲染且不产生操作按钮', () => {
    const wrapper = render(' \n ')
    expect(wrapper.get('.update-markdown__content').text()).toBe('')
    expect(wrapper.find('button').exists()).toBe(false)
  })

  it('点击链接内的强调文本只请求系统打开一次', async () => {
    let finish!: () => void
    vi.mocked(downloadsApi.openUpdateLink).mockReturnValue(new Promise(resolve => { finish = resolve }))
    const wrapper = render()
    await wrapper.get('[role="link"] strong').trigger('click')
    await wrapper.get('[role="link"]').trigger('click')
    expect(downloadsApi.openUpdateLink).toHaveBeenCalledExactlyOnceWith(comparison)
    expect(wrapper.text()).toContain('正在打开链接')
    finish(); await flushPromises()
    expect(wrapper.find('[role="status"]').exists()).toBe(false)
  })

  it('打开失败保留正文，显示局部错误并允许重试', async () => {
    vi.mocked(downloadsApi.openUpdateLink).mockRejectedValueOnce(new Error('无法启动浏览器')).mockResolvedValueOnce(undefined)
    const wrapper = render()
    await wrapper.get('[role="link"]').trigger('click'); await flushPromises()
    expect(wrapper.get('[role="alert"]').text()).toBe('无法启动浏览器')
    expect(wrapper.get('h2').text()).toBe('验证')
    await wrapper.get('[role="link"]').trigger('click'); await flushPromises()
    expect(downloadsApi.openUpdateLink).toHaveBeenCalledTimes(2)
    expect(wrapper.find('[role="alert"]').exists()).toBe(false)
  })

  it('后端未就绪或鼠标右键不会发起链接打开', async () => {
    const wrapper = render(notes, false)
    await wrapper.get('[role="link"]').trigger('click')
    await wrapper.setProps({ ready: true })
    await wrapper.get('[role="link"]').trigger('auxclick', { button: 2 })
    expect(downloadsApi.openUpdateLink).not.toHaveBeenCalled()
  })

  it('换入新正文后不展示旧打开请求的错误', async () => {
    let reject!: (error: Error) => void
    vi.mocked(downloadsApi.openUpdateLink).mockReturnValue(new Promise((_, fail) => { reject = fail }))
    const wrapper = render()
    await wrapper.get('[role="link"]').trigger('click')
    await wrapper.setProps({ notes: '# 新版本说明' })
    reject(new Error('旧请求错误')); await flushPromises()
    expect(wrapper.get('h1').text()).toBe('新版本说明')
    expect(wrapper.find('[role="alert"]').exists()).toBe(false)
  })

  it('组件卸载后处理未完成请求的拒绝，不残留状态', async () => {
    let reject!: (error: Error) => void
    vi.mocked(downloadsApi.openUpdateLink).mockReturnValue(new Promise((_, fail) => { reject = fail }))
    const wrapper = render()
    await wrapper.get('[role="link"]').trigger('click')
    wrapper.unmount()
    reject(new Error('迟到的错误')); await flushPromises()
    expect(document.querySelector('[role="alert"]')).toBeNull()
  })

  it('实际更新视图使用 Markdown，官方发布页失败仍保留正文', async () => {
    vi.mocked(downloadsApi.openRelease).mockRejectedValue('打开发布页失败')
    const wrapper = await renderView()
    expect(wrapper.get('.update-markdown h2').text()).toBe('验证')
    await wrapper.get('.update-release button').trigger('click'); await flushPromises()
    expect(downloadsApi.openRelease).toHaveBeenCalledExactlyOnceWith(result.url)
    expect(wrapper.get('.update-markdown h2').text()).toBe('验证')
    expect(wrapper.get('.update-release [role="alert"]').text()).toBe('打开发布页失败')
  })

  it.each([null, ''])('说明为空 %s 时仍显示版本检查结果', async source => {
    vi.mocked(downloadsApi.checkUpdate).mockResolvedValue({ ...result, notes: source })
    const wrapper = await renderView()
    expect(wrapper.text()).toContain('当前版本无需更新')
    expect(wrapper.findComponent(UpdateNotes).exists()).toBe(false)
  })

  it('限流等待时间与检查失败仍可理解', async () => {
    vi.mocked(downloadsApi.checkUpdate).mockResolvedValue({ ...result, status: 'rate_limited', notes: null, message: '请求过于频繁', nextCheck: 1791380000000 })
    const wrapper = await renderView()
    expect(wrapper.text()).toContain('请求过于频繁')
    expect(wrapper.text()).toContain('下次可检查')
    vi.mocked(downloadsApi.checkUpdate).mockRejectedValue(new Error('网络不可用'))
    await wrapper.get('.update-check').trigger('click'); await flushPromises()
    expect(wrapper.get('[role="alert"]').text()).toBe('网络不可用')
  })
})

describe('更新说明链接边界', () => {
  it.each([
    comparison,
    'https://github.com/test/repo/releases/tag/v1',
    'https://github.com/test/repo/issues/1#issuecomment-1',
    'https://github.com/test/repo/pull/1?diff=split',
    'https://GITHUB.COM:443/test/repo',
    'https://github.com/test/repo/blob/main/%E4%B8%AD%E6%96%87%20test.md',
  ])('接受 HTTPS GitHub 地址 %s', value => {
    expect(normalizeUpdateLink(value)).toBe(new URL(value).href)
  })

  it.each([
    '', ' ', 'not a url', '/test/repo', '//github.com/test/repo', 'https:github.com/test/repo',
    'http://github.com/test/repo', 'javascript:alert(1)', 'file:///C:/test.exe',
    'https://github.com.evil/test/repo', 'https://evil.github.com/test/repo', 'https://github.com./test/repo',
    'https://user:pass@github.com/test/repo', 'https://@github.com/test/repo', 'https://github.com:8443/test/repo',
    'https://github.com\\@evil.test', 'https://git\nhub.com/test/repo', ' https://github.com/test/repo',
    'https://github.com/test/repo\u0000', 'https://github.com/%0A', 'https://github.com/%', 'https://github.com/%FF',
  ])('拒绝无效或越界地址 %s', value => {
    expect(normalizeUpdateLink(value)).toBeNull()
  })
})
