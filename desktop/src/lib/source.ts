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
