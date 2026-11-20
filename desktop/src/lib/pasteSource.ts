import type { ClipboardEvent } from "react";
import { request } from "./api";
import type { LinkSuggestion } from "./clipboard";

/**
 * 粘贴进地址输入框时，把整段简介、跳转链接、加速站链接等换成识别出的 GitHub 地址；
 * 识别不出时按原样粘贴。识别在 Rust 端完成，和剪贴板横幅用同一套规则。
 */
export function pasteSource(
  event: ClipboardEvent<HTMLTextAreaElement | HTMLInputElement>,
  apply: (value: string) => void,
) {
  const text = event.clipboardData.getData("text/plain");
  if (!text.trim()) return;
  const field = event.currentTarget;
  const start = field.selectionStart ?? field.value.length;
  const end = field.selectionEnd ?? field.value.length;
  const before = field.value.slice(0, start);
  const after = field.value.slice(end);
  event.preventDefault();
  request<LinkSuggestion | null>("recognize", { text })
    .catch(() => null)
    .then((suggestion) => apply(before + (suggestion?.source ?? text) + after));
}
