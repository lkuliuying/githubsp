import { useEffect, useRef, useState } from "react";
import { RefreshCw } from "lucide-react";
import { Empty, useJob } from "@/components/common";
import { Pager, clampPage, usePageSize } from "@/components/Pager";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Modal } from "@/components/ui/dialog";
import { request, useWorkspace } from "@/lib/api";
import { date, type UpdateEvent, type UpdatePage } from "@/lib/model";
import { Markdown } from "./NewDownload";

/** 「我的软件 → 更新动态」：按时间列出关注项目发现的新版本。 */
export function UpdateFeed({ active: shown }: { active: boolean }) {
  const { snapshot, connected } = useWorkspace();
  const [page, setPage] = useState<UpdatePage | null>(null);
  const [events, setEvents] = useState<UpdateEvent[]>([]);
  const [query, setQuery] = useState("");
  const [current, setCurrent] = useState(1);
  const [pageSize, setPageSize] = usePageSize("feed");
  const [group, setGroup] = useState("");
  const [selected, setSelected] = useState<UpdateEvent | null>(null);
  const [reading, setReading] = useState(false);
  const { busy, run, active } = useJob();
  const epoch = useRef(0);
  // 检查发现新版本后最新标签会变化，此时重新读取动态。
  const latest = snapshot?.favorites
    .map((favorite) => `${favorite.repository}:${favorite.latest?.tag}`)
    .join("|");
  async function load(more = false) {
    const generation = ++epoch.current;
    const next = await request<UpdatePage>("updates", {
      cursor: more ? page?.nextCursor : null,
      limit: 100,
      repository: null,
      recent: true,
    });
    if (!active.current || generation !== epoch.current) return;
    setPage(next);
    setEvents((old) => (more ? [...old, ...next.events] : next.events));
  }
  useEffect(() => {
    if (shown && connected) run(() => load());
    return () => {
      epoch.current++;
      setReading(false);
      setSelected(null);
    };
  }, [shown, connected, latest]);
  async function detail(id: number) {
    const generation = ++epoch.current;
    setReading(true);
    try {
      const event = await request<UpdateEvent>("updateDetail", { id });
      if (active.current && generation === epoch.current) setSelected(event);
    } finally {
      if (generation === epoch.current) setReading(false);
    }
  }
  async function action(event: UpdateEvent, value: string) {
    await request("updateAction", { id: event.id, action: value });
    const fresh = await request<UpdateEvent>("updateDetail", { id: event.id });
    if (!active.current) return;
    setEvents((old) =>
      old.map((item) =>
        item.id === fresh.id ? { ...fresh, notes: null } : item,
      ),
    );
    setSelected((old) => (old?.id === fresh.id ? fresh : old));
  }
  const groups = [
    ...new Set(
      Object.values(snapshot?.watchState.repositories ?? {})
        .map((entry) => entry.group)
        .filter((value): value is string => !!value),
    ),
  ];
  const filtered = events
    .filter((event) => {
      const labels =
        snapshot?.watchState.repositories[event.repository.toLowerCase()];
      return (
        (!group || labels?.group === group) &&
        `${event.repository} ${event.release.tag} ${labels?.tags?.join(" ") ?? ""}`
          .toLowerCase()
          .includes(query.toLowerCase())
      );
    })
    .sort((a, b) => b.id - a.id);
  const listPage = clampPage(current, filtered.length, pageSize);
  return (
    <div className="feed">
      <div className="sw-toolbar">
        <Input
          aria-label="搜索更新动态"
          placeholder="搜索项目、版本或标签"
          value={query}
          onChange={(event) => {
            setQuery(event.target.value);
            setCurrent(1);
          }}
        />
        {groups.length > 0 && (
          <select
            aria-label="按分组筛选更新动态"
            value={group}
            onChange={(event) => setGroup(event.target.value)}
          >
            <option value="">全部分组</option>
            {groups.map((value) => (
              <option key={value}>{value}</option>
            ))}
          </select>
        )}
        <Button
          size="sm"
          variant="ghost"
          disabled={busy || !connected}
          onClick={() => run(() => load())}
        >
          <RefreshCw size={14} />
          刷新
        </Button>
      </div>
      {!filtered.length ? (
        <Empty
          title={events.length ? "没有匹配的动态" : "还没有新版本动态"}
          description="关注项目后，之后发布的新版本会按时间出现在这里。"
        />
      ) : (
        <div className="update-list">
          {filtered
            .slice((listPage - 1) * pageSize, listPage * pageSize)
            .map((event) => (
            <article
              className={`update-row ${!event.seenAt && !event.skipped ? "is-unread" : ""}`}
              key={event.id}
            >
              <button
                className="update-open"
                disabled={reading}
                onClick={() => run(() => detail(event.id))}
              >
                <strong>
                  {!event.seenAt && !event.skipped && (
                    <i className="unread-dot" aria-label="未读" />
                  )}
                  {event.repository}
                </strong>
                <span>
                  {event.release.tag}
                  {event.prerelease ? " · 预发布" : ""}
                </span>
                <small>
                  {date(event.release.publishedAt ?? event.discoveredAt)}
                </small>
              </button>
              <div className="update-state">
                {event.skipped && <span>已忽略</span>}
                {event.downloadedTasks.length > 0 && <span>已下载</span>}
                {!event.metadataComplete && (
                  <span title="此版本来自降级更新源，附件列表可能不完整">
                    附件信息不完整
                  </span>
                )}
              </div>
              <div className="update-actions">
                {!event.seenAt && (
                  <Button
                    size="sm"
                    disabled={busy}
                    onClick={() => run(() => action(event, "seen"))}
                  >
                    标为已读
                  </Button>
                )}
                <Button
                  size="sm"
                  variant="ghost"
                  disabled={busy}
                  onClick={() =>
                    run(() => action(event, event.skipped ? "unskip" : "skip"))
                  }
                >
                  {event.skipped ? "取消忽略" : "忽略此版本"}
                </Button>
              </div>
            </article>
          ))}
        </div>
      )}
      {filtered.length > 0 && (
        <Pager
          page={listPage}
          total={filtered.length}
          pageSize={pageSize}
          onPage={setCurrent}
          onPageSize={setPageSize}
          unit="条动态"
          extra={
            page?.hasMore &&
            listPage * pageSize >= filtered.length && (
              <Button size="sm" variant="ghost" disabled={busy} onClick={() => run(() => load(true))}>
                加载更早的动态
              </Button>
            )
          }
        />
      )}
      <Modal
        open={!!selected}
        busy={busy}
        onClose={() => {
          epoch.current++;
          setSelected(null);
        }}
        title={
          selected
            ? `${selected.repository} · ${selected.release.tag}`
            : "版本详情"
        }
      >
        {selected && (
          <>
            <p className="form-note">
              {date(selected.release.publishedAt ?? selected.discoveredAt)}
            </p>
            <Markdown>
              {selected.notes ?? "没有获取到完整说明，请打开发布页查看。"}
            </Markdown>
            <Button
              onClick={() =>
                run(() => request("openLink", { url: selected.release.url }))
              }
            >
              打开发布页
            </Button>
          </>
        )}
      </Modal>
    </div>
  );
}
