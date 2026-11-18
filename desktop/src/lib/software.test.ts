import { describe, expect, it } from "vitest";
import { buildProjects } from "./software";
import type { Favorite, LocalSoftware, Snapshot } from "./model";

const favorite = (repository: string, tag: string | null, id = 1): Favorite => ({
  id: repository,
  repository,
  latest: tag ? { id, tag, url: "", publishedAt: 1 } : null,
  lastChecked: 1,
  nextCheck: null,
  error: null,
});
const local = (repository: string, tag: string): LocalSoftware => ({
  repository,
  taskId: repository,
  tag,
  filename: "a.zip",
  path: "C:/a.zip",
  bytes: 1,
  verification: "unverified",
  completedAt: 2,
  fileState: "present",
  fileCount: 1,
});
const snapshot = (favorites: Favorite[], repositories = {}) =>
  ({ favorites, watchState: { repositories } }) as unknown as Snapshot;

describe("我的软件：收藏与本地下载合并", () => {
  it("按本地与最新版本判断状态，可更新的排在最前", () => {
    const projects = buildProjects(
      snapshot([
        favorite("a/current", "v1"),
        favorite("a/update", "v2"),
        favorite("a/fresh", "v3"),
        favorite("a/pending", null),
      ]),
      [local("A/Current", "v1"), local("a/update", "v1"), local("x/loose", "v9")],
    );
    expect(projects.map((project) => [project.key, project.state])).toEqual([
      ["a/update", "update"],
      ["a/fresh", "fresh"],
      ["a/pending", "pending"],
      ["a/current", "current"],
      ["x/loose", "unwatched"],
    ]);
  });

  it("忽略的最新版本不再显示为可更新，文件异常单独标记", () => {
    const [project] = buildProjects(
      snapshot([favorite("a/b", "v2", 7)], {
        "a/b": { skippedReleaseIds: [7], ackedReleaseId: null, rule: {} },
      }),
      [{ ...local("a/b", "v1"), fileState: "missing" }],
    );
    expect(project.state).toBe("current");
    expect(project.skipped).toBe(true);
    expect(project.unread).toBe(false);
    expect(project.fileIssue).toBe(true);
  });
});
