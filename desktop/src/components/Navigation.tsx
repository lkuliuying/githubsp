// 侧边栏滑块的弹簧动画在 useSpringThumb 里，改编自 yetone/magpie（MIT），见 THIRD_PARTY_NOTICES.md。
import {
  Download,
  Plus,
  LayoutGrid,
  Settings,
} from "lucide-react";
import { navigate, useWorkspace } from "@/lib/api";
import { useSpringThumb } from "@/lib/useSpringThumb";
import { RollingNumber } from "@/components/RollingNumber";
import { unread } from "@/lib/model";

const items = [
  { view: "queue", label: "下载队列", icon: Download },
  { view: "new", label: "新建下载", icon: Plus },
  { view: "software", label: "我的软件", icon: LayoutGrid },
  { view: "settings", label: "设置", icon: Settings },
] as const;

export function Navigation({ reduced }: { reduced: boolean }) {
  const { view, snapshot } = useWorkspace();
  const { root, thumb } = useSpringThumb<HTMLElement>(view, {
    axis: "y",
    selector: ":scope > .active",
    reduced,
  });
  const updates =
    snapshot?.favorites.filter((favorite) => unread(snapshot, favorite))
      .length ?? 0;
  return (
    <aside className="sidebar">
      <nav aria-label="主导航" ref={root}>
        <span className="nav-top-hl" ref={thumb} aria-hidden="true" />
        {items.map(({ view: key, label, icon: Icon }) => (
          <button
            key={key}
            data-view={key}
            aria-current={view === key ? "page" : undefined}
            className={`nav-item ${view === key ? "active" : ""} ${key === "settings" ? "nav-settings" : ""}`}
            onClick={() => navigate(key)}
          >
            <Icon size={16} strokeWidth={1.6} />
            <span>{label}</span>
            {key === "queue" &&
              !!snapshot?.tasks.filter(
                (task) => !["completed", "cancelled"].includes(task.status),
              ).length && (
                <small>
                  <RollingNumber
                    value={
                      snapshot.tasks.filter(
                        (task) =>
                          !["completed", "cancelled"].includes(task.status),
                      ).length
                    }
                  />
                </small>
              )}
            {key === "software" && updates > 0 && (
              <small>
                <RollingNumber value={updates} />
              </small>
            )}
          </button>
        ))}
      </nav>
    </aside>
  );
}
