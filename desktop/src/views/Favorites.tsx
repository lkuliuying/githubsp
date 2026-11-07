import { PageHeader } from "@/components/PageActivity";
import { useState } from "react";
import {
  ArrowDownToLine,
  ArrowUpRight,
  Check,
  GitBranch,
  Plus,
  RefreshCw,
  Search,
  SlidersHorizontal,
  Star,
  Trash2,
} from "lucide-react";
import { navigate, notify, request, useWorkspace } from "@/lib/api";
import { date, unread, type Favorite, type WatchRule } from "@/lib/model";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Modal } from "@/components/ui/dialog";
import { Empty, Toggle, useConfirm, useJob } from "@/components/common";
import { openRepository } from "./NewDownload";

export function Favorites() {
  const { snapshot, connected } = useWorkspace();
  const [query, setQuery] = useState("");
  const [onlyUpdates, setOnlyUpdates] = useState(false);
  const [adding, setAdding] = useState(false);
  const [input, setInput] = useState("");
  const [editing, setEditing] = useState<Favorite | null>(null);
  const [rule, setRule] = useState<WatchRule>({
    includePrerelease: false,
    autoDownload: false,
    assetPattern: null,
  });
  const { busy, run, active } = useJob();
  const confirm = useConfirm();
  const favorites =
    snapshot?.favorites.filter(
      (f) =>
        f.repository.toLowerCase().includes(query.toLowerCase()) &&
        (!onlyUpdates || unread(snapshot, f)),
    ) ?? [];
  const updates =
    snapshot?.favorites.filter((f) => unread(snapshot, f)).length ?? 0;
  return (
    <section className="page favorites-page" tabIndex={-1}>
      <PageHeader>
        <div>
          <h1>
            项目收藏
            {updates > 0 && (
              <span className="count-label accent">{updates} 项更新</span>
            )}
          </h1>
        </div>
        <div className="header-actions">
          <Button
            disabled={
              busy ||
              !connected ||
              !snapshot?.favorites.length ||
              snapshot?.checking.active
            }
            onClick={() =>
              run(() => request("favoriteCheck", { repository: null }))
            }
          >
            <RefreshCw
              size={15}
              className={snapshot?.checking.active ? "spin" : ""}
            />
            检查更新
          </Button>
          <Button
            variant="default"
            disabled={!connected}
            onClick={() => setAdding(true)}
          >
            <Plus size={16} />
            收藏项目
          </Button>
        </div>
      </PageHeader>
      {snapshot?.checking.active && (
        <div className="checking-banner">
          <RefreshCw size={16} className="spin" />
          <span>
            正在检查 {snapshot.checking.current} · {snapshot.checking.done}/
            {snapshot.checking.total}
          </span>
          <Button size="sm" variant="ghost" onClick={() => runCancel()}>
            取消检查
          </Button>
        </div>
      )}
      <div className="favorites-toolbar">
        <label className="search-box">
          <Search size={15} />
          <Input
            aria-label="搜索收藏项目"
            placeholder="搜索 owner/repository"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
          />
        </label>
        <button
          className={`filter-chip ${onlyUpdates ? "selected" : ""}`}
          onClick={() => setOnlyUpdates(!onlyUpdates)}
        >
          <span className="live-dot" />
          只看有更新
        </button>
        <span className="spacer" />
        {updates > 0 && (
          <Button
            variant="ghost"
            size="sm"
            disabled={busy}
            onClick={() =>
              run(() => request("favoriteDismiss", { repository: null }))
            }
          >
            <Check size={14} />
            全部标为已读
          </Button>
        )}
      </div>
      <div className="favorites-grid">
        {favorites.map((favorite) => {
          const entry =
            snapshot!.watchState.repositories[
              favorite.repository.toLowerCase()
            ];
          const isUnread = unread(snapshot!, favorite);
          return (
            <article
              className={`favorite-card ${isUnread ? "has-update" : ""}`}
              key={favorite.id}
            >
              <div className="favorite-top">
                <div className="repository-mark">
                  <GitBranch size={18} />
                </div>
                <div className="favorite-name">
                  <small>{favorite.repository.split("/")[0]}</small>
                  <h2 title={favorite.repository}>
                    {favorite.repository.split("/").slice(1).join("/")}
                  </h2>
                </div>
                <Star size={17} className="favorite-star" />
              </div>
              <div className="favorite-version">
                <span>{favorite.latest?.tag || "等待检查"}</span>
                {isUnread && (
                  <span className="status status-downloading">新版本</span>
                )}
              </div>
              <p className="favorite-notes">
                {favorite.error ||
                  entry?.notesExcerpt ||
                  "收藏已保存。检查更新后，这里会显示最新版本摘要。"}
              </p>
              <div className="favorite-meta">
                <span>
                  发布于{" "}
                  {date(entry?.publishedAt || favorite.latest?.publishedAt)}
                </span>
                <span>上次检查 {date(favorite.lastChecked)}</span>
              </div>
              <div className="favorite-actions">
                <Button
                  size="sm"
                  disabled={busy || !favorite.latest}
                  onClick={() =>
                    run(() =>
                      request("favoriteDownload", {
                        repository: favorite.repository,
                      }),
                    )
                  }
                >
                  <ArrowDownToLine size={14} />
                  下载最新版
                </Button>
                <Button
                  size="sm"
                  variant="ghost"
                  onClick={() => openRepository(favorite.repository)}
                >
                  查看附件
                  <ArrowUpRight size={14} />
                </Button>
                <div className="spacer" />
                <Button
                  size="icon"
                  variant="ghost"
                  aria-label={`设置 ${favorite.repository} 的关注规则`}
                  onClick={() => {
                    setEditing(favorite);
                    setRule(
                      entry?.rule ?? {
                        includePrerelease: false,
                        autoDownload: false,
                        assetPattern: null,
                      },
                    );
                  }}
                >
                  <SlidersHorizontal size={15} />
                </Button>
              </div>
              <div className="favorite-secondary">
                <button
                  disabled={busy || snapshot?.checking.active}
                  onClick={() =>
                    run(() =>
                      request("favoriteCheck", {
                        repository: favorite.repository,
                      }),
                    )
                  }
                >
                  立即检查
                </button>
                {isUnread && (
                  <button
                    disabled={busy}
                    onClick={() =>
                      run(() =>
                        request("favoriteDismiss", {
                          repository: favorite.repository,
                        }),
                      )
                    }
                  >
                    忽略本次更新
                  </button>
                )}
                <button
                  disabled={busy}
                  className="danger-text"
                  onClick={() =>
                    run(async () => {
                      if (
                        await confirm(
                          "取消收藏这个项目？",
                          favorite.repository,
                          "取消收藏",
                        )
                      )
                        await request("favoriteRemove", {
                          repository: favorite.repository,
                        });
                    })
                  }
                >
                  <Trash2 size={12} />
                  取消收藏
                </button>
              </div>
            </article>
          );
        })}
      </div>
      {!favorites.length && (
        <Empty
          title={query || onlyUpdates ? "没有符合条件的项目" : "尚未收藏项目"}
          description="收藏 GitHub 项目，集中查看版本与更新摘要。"
          action={
            !query && (
              <Button disabled={!connected} onClick={() => setAdding(true)}>
                <Plus size={15} />
                收藏项目
              </Button>
            )
          }
        />
      )}
      <div className="page-footnote">
        <GitBranch size={14} />
        {snapshot?.settings.autoCheck
          ? `自动检查：每 ${snapshot.settings.watchIntervalMs < 3_600_000 ? `${snapshot.settings.watchIntervalMs / 60_000} 分钟` : `${snapshot.settings.watchIntervalMs / 3_600_000} 小时`} 一次`
          : "当前为手动检查。"}
        <button onClick={() => navigate("settings")}>管理检查设置 →</button>
      </div>
      <Modal
        open={adding}
        onClose={() => setAdding(false)}
        busy={busy}
        title="收藏 GitHub 项目"
        description="粘贴公开仓库或 Release 页面地址。"
        footer={
          <>
            <Button disabled={busy} onClick={() => setAdding(false)}>
              取消
            </Button>
            <Button
              variant="default"
              disabled={busy || !input.trim()}
              onClick={() =>
                run(async () => {
                  await request("favoriteAdd", { input: input.trim() });
                  if (active.current) {
                    setAdding(false);
                    setInput("");
                  }
                })
              }
            >
              {busy ? "正在添加并检查…" : "添加收藏"}
            </Button>
          </>
        }
      >
        <label className="field-label" htmlFor="favorite-url">
          仓库地址
        </label>
        <Input
          id="favorite-url"
          placeholder="https://github.com/owner/repository"
          value={input}
          disabled={busy}
          onChange={(e) => setInput(e.target.value)}
          autoFocus
        />
      </Modal>
      <Modal
        open={!!editing}
        onClose={() => setEditing(null)}
        busy={busy}
        title="项目关注规则"
        description={editing?.repository}
        footer={
          <>
            <Button disabled={busy} onClick={() => setEditing(null)}>
              取消
            </Button>
            <Button
              variant="default"
              disabled={busy}
              onClick={() =>
                run(async () => {
                  await request("watchRule", {
                    repository: editing!.repository,
                    rule,
                  });
                  if (active.current) setEditing(null);
                })
              }
            >
              保存规则
            </Button>
          </>
        }
      >
        <Toggle
          title="包含预发布版本"
          checked={rule.includePrerelease}
          onChange={(v) => setRule({ ...rule, includePrerelease: v })}
        />
        <Toggle
          title="自动下载新版本"
          description="自动检查开启时生效，附件仍按下载队列处理。"
          checked={rule.autoDownload}
          onChange={(v) => setRule({ ...rule, autoDownload: v })}
        />
        <label className="field-label" htmlFor="watch-pattern">
          附件名称匹配
        </label>
        <Input
          id="watch-pattern"
          placeholder="例如 *windows*x64*.zip，留空使用全局规则"
          value={rule.assetPattern || ""}
          onChange={(e) =>
            setRule({ ...rule, assetPattern: e.target.value || null })
          }
        />
        <p className="form-note">支持 *、? 通配符和 {"{version}"} 占位符。</p>
      </Modal>
    </section>
  );
  async function runCancel() {
    try {
      await request("favoriteCancel");
    } catch (error) {
      notify(String(error), "error");
    }
  }
}
