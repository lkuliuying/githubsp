import MarkdownIt, { type Token } from 'markdown-it'

const markdown = new MarkdownIt({ html: false, linkify: true, typographer: false })

/** 与 Rust 的更新链接边界保持一致，返回可交给系统浏览器的规范地址。 */
export function normalizeUpdateLink(value: string): string | null {
  if (!/^https:\/\//i.test(value) || /[\s\u0000-\u001f\u007f-\u009f\\]/u.test(value)) return null
  const authority = value.slice(value.indexOf('://') + 3).split(/[/?#]/, 1)[0] || ''
  if (authority.includes('@')) return null
  try {
    if (/[\u0000-\u001f\u007f-\u009f]/u.test(decodeURIComponent(value))) return null
    const url = new URL(value)
    if (url.protocol !== 'https:' || url.hostname !== 'github.com' || url.port || url.username || url.password) return null
    return url.href
  } catch {
    return null
  }
}

function prepareLinks(tokens: Token[]) {
  const tags: string[] = []
  for (const token of tokens) {
    if (token.type === 'link_open') {
      const href = token.attrGet('href')
      const url = normalizeUpdateLink(typeof href === 'string' ? href : '')
      // 使用按钮承载外链，避免 WebView 的默认导航及右键打开绕过系统接口。
      token.tag = url ? 'button' : 'span'
      token.attrs = url
        ? [['type', 'button'], ['role', 'link'], ['data-update-url', url], ['title', url]]
        : null
      tags.push(token.tag)
    } else if (token.type === 'link_close') {
      token.tag = tags.pop() || 'span'
    }
    if (token.children) prepareLinks(token.children)
  }
}

markdown.renderer.rules.image = (tokens, index, options, env, renderer) => {
  const text = renderer.renderInlineAsText(tokens[index]!.children || [], options, env)
  return markdown.utils.escapeHtml(text || '图片')
}

/** 只渲染解析器生成的标签；原始 HTML 保留为文本，图片不创建网络请求。 */
export function renderUpdateNotes(source: string): string {
  const env = {}
  const tokens = markdown.parse(source, env)
  prepareLinks(tokens)
  return markdown.renderer.render(tokens, markdown.options, env)
}
