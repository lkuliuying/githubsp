import { describe, expect, it } from "vitest";
import {
  bytes,
  parseLimit,
  progress,
  settingsFingerprint,
  type Settings,
  type Task,
} from "./model";

describe("前端与 Rust 核心的显示及限速契约", () => {
  it("设置比较忽略 IPC 序列化的键顺序，但仍识别字段值变化", () => {
    const saved = {
      limitKib: 1000,
      closeToTray: false,
      reduceMotion: false,
    } as Settings;
    const pushed = {
      reduceMotion: false,
      closeToTray: false,
      limitKib: 1000,
    } as Settings;
    expect(settingsFingerprint(saved)).toBe(settingsFingerprint(pushed));
    expect(settingsFingerprint(saved)).not.toBe(
      settingsFingerprint({ ...pushed, closeToTray: true }),
    );
  });
  it("保留原始 KiB 限速精度，并拒绝空值及无效范围", () => {
    expect(parseLimit("1.024", 1000)).toBe(1000);
    expect(parseLimit("0.001024", 0)).toBe(1);
    expect(parseLimit(" 0 ", 10)).toBe(0);
    for (const input of ["", "NaN", "Infinity", "-1", "0.0001", "10241"])
      expect(() => parseLimit(input, 0)).toThrow();
  });
  it("未知大小不显示伪造的百分比，超出进度限制为完整进度", () => {
    expect(progress({ total: null, downloaded: 100 } as Task)).toBeNull();
    expect(progress({ total: 200, downloaded: 100 } as Task)).toBe(50);
    expect(progress({ total: 200, downloaded: 201 } as Task)).toBe(100);
    expect(bytes(null)).toBe("大小未知");
    expect(bytes(0)).toBe("0 B");
  });
});
