import { describe, expect, it } from "vitest";
import { detectSource, sourceArchiveUrl } from "./source";

describe("新建下载：识别粘贴的内容", () => {
  it("区分仓库地址、附件链接和混合输入", () => {
    expect(detectSource("  ").kind).toBe("empty");
    expect(detectSource("https://github.com/o/r").kind).toBe("repository");
    expect(detectSource("https://github.com/o/r/releases/tag/v1").kind).toBe(
      "repository",
    );
    expect(
      detectSource(
        "https://github.com/o/r/releases/download/v1/a.zip\r\n\n https://github.com/o/r/releases/latest/download/b.exe ",
      ),
    ).toEqual({
      kind: "links",
      lines: [
        "https://github.com/o/r/releases/download/v1/a.zip",
        "https://github.com/o/r/releases/latest/download/b.exe",
      ],
    });
    expect(
      detectSource(
        "https://github.com/o/r\nhttps://github.com/o/r/releases/download/v1/a.zip",
      ).kind,
    ).toBe("mixed");
  });
});

describe("源码压缩包地址", () => {
  it("默认分支与带斜杠、特殊字符的版本号", () => {
    expect(sourceArchiveUrl("o/r")).toBe("https://github.com/o/r/archive/HEAD.zip");
    expect(sourceArchiveUrl("o/r", "v1.2.0")).toBe(
      "https://github.com/o/r/archive/refs/tags/v1.2.0.zip",
    );
    expect(sourceArchiveUrl("o/r", "release/1.0 beta#2")).toBe(
      "https://github.com/o/r/archive/refs/tags/release/1.0%20beta%232.zip",
    );
  });
});
