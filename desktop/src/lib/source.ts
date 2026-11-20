/** 附件直链：.../releases/download/{tag}/{file} 或 .../releases/latest/download/{file}。 */
export const isAssetLink = (line: string) =>
  /\/releases\/(latest\/)?download\//i.test(line);

/** 判断粘贴的内容：一个仓库/Release 页面地址，或若干附件直链。 */
export function detectSource(source: string) {
  const lines = source
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter(Boolean);
  if (!lines.length) return { kind: "empty" as const, lines };
  if (lines.every(isAssetLink)) return { kind: "links" as const, lines };
  if (lines.length === 1) return { kind: "repository" as const, lines };
  return { kind: "mixed" as const, lines };
}

/**
 * GitHub 自动生成的源码压缩包地址。不传版本号时取默认分支的最新代码。
 * 版本号里可能有斜杠（如 release/1.0），按路径逐段编码。
 */
export function sourceArchiveUrl(repository: string, tag?: string) {
  const ref = tag
    ? `refs/tags/${tag.split("/").map(encodeURIComponent).join("/")}`
    : "HEAD";
  return `https://github.com/${repository}/archive/${ref}.zip`;
}
