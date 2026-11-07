import { describe, expect, it, vi } from "vitest";

const native = vi.hoisted(() => ({ invoke: vi.fn(), listen: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({
  invoke: native.invoke,
  isTauri: () => true,
}));
vi.mock("@tauri-apps/api/event", () => ({ listen: native.listen }));

describe("窗口订阅的生命周期", () => {
  it("初始化中卸载也释放迟到的监听器，不再请求本地数据", async () => {
    const stop = vi.fn();
    let ready!: (value: () => void) => void;
    native.listen
      .mockImplementationOnce(
        () =>
          new Promise((resolve) => {
            ready = resolve;
          }),
      )
      .mockResolvedValue(stop);
    const { connect } = await import("./api");
    const controller = new AbortController();
    const pending = connect(controller.signal);
    controller.abort();
    ready(stop);
    await pending;
    expect(native.invoke).not.toHaveBeenCalled();
    expect(stop).toHaveBeenCalledTimes(3);
  });
});
