import { describe, expect, it } from "vitest";
import { detectSource } from "./source";

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
