/** Rust 端从剪贴板识别出的 GitHub 下载来源；原始剪贴板文字不会传到界面。 */
export interface LinkSuggestion {
  kind: "repository" | "links";
  /** 填入新建下载输入框的内容；附件链接按行分隔。 */
  source: string;
  repository: string;
  version?: string | null;
  latest: boolean;
  count: number;
  filename?: string | null;
}

export interface ClipboardResult {
  sequence: number;
  suggestion: LinkSuggestion | null;
}

/** 横幅上的说明文字与主按钮文字。 */
export function describeSuggestion(s: LinkSuggestion) {
  if (s.kind === "links") {
    return {
      title:
        s.count === 1 && s.filename
          ? `剪贴板里有附件链接：${s.filename}`
          : `剪贴板里有 ${s.count} 个附件链接`,
      detail: s.version ? `${s.repository} · ${s.version}` : s.repository,
      action: "去下载",
    };
  }
  return {
    title: `剪贴板里有 GitHub 链接：${s.repository}`,
    detail: s.version ? s.version : s.latest ? "最新版本" : "",
    action: "读取版本",
  };
}

/** Ctrl+V 是否应交给输入控件处理。 */
export function isEditableTarget(target: EventTarget | null) {
  if (!(target instanceof HTMLElement)) return false;
  if (target.isContentEditable) return true;
  if (
    target instanceof HTMLTextAreaElement ||
    target instanceof HTMLSelectElement
  )
    return true;
  if (target instanceof HTMLInputElement)
    return ![
      "checkbox",
      "radio",
      "button",
      "submit",
      "reset",
      "range",
      "color",
      "file",
    ].includes(target.type);
  return false;
}
