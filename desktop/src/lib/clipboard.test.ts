import { describe, expect, it } from "vitest";
import { describeSuggestion, type LinkSuggestion } from "./clipboard";

const base: LinkSuggestion = {
  kind: "repository",
  source: "https://github.com/o/r",
  repository: "o/r",
  version: null,
  latest: false,
  count: 0,
  filename: null,
};

describe("剪贴板链接提示文字", () => {
  it("仓库页显示仓库名，版本页和最新版本页带上版本说明", () => {
    expect(describeSuggestion(base)).toEqual({
      title: "剪贴板里有 GitHub 链接：o/r",
      detail: "",
      action: "读取版本",
    });
    expect(describeSuggestion({ ...base, version: "v1.2.0" }).detail).toBe(
      "v1.2.0",
    );
    expect(describeSuggestion({ ...base, latest: true }).detail).toBe(
      "最新版本",
    );
  });
  it("附件链接显示文件名或数量", () => {
    const one = describeSuggestion({
      ...base,
      kind: "links",
      count: 1,
      filename: "app.exe",
      version: "v1",
    });
    expect(one.title).toBe("剪贴板里有附件链接：app.exe");
    expect(one.detail).toBe("o/r · v1");
    expect(one.action).toBe("去下载");
    expect(describeSuggestion({ ...base, kind: "links", count: 3 }).title).toBe(
      "剪贴板里有 3 个附件链接",
    );
  });
});
