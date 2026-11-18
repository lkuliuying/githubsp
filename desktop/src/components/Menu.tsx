import { useEffect, useRef, useState, type ReactNode } from "react";
import { MoreHorizontal } from "lucide-react";
import { Button } from "./ui/button";

export interface MenuItem {
  label: string;
  onSelect(): void;
  icon?: ReactNode;
  disabled?: boolean;
  danger?: boolean;
}

/** 轻量的「⋯」操作菜单：点击外部或按 Esc 关闭，方向键在项目间移动。 */
export function Menu({
  label,
  items,
  disabled = false,
}: {
  label: string;
  items: (MenuItem | false | null | undefined)[];
  disabled?: boolean;
}) {
  const [open, setOpen] = useState(false);
  const [up, setUp] = useState(false);
  const root = useRef<HTMLDivElement>(null);
  const trigger = useRef<HTMLButtonElement>(null);
  const shown = items.filter((item): item is MenuItem => !!item);
  useEffect(() => {
    if (!open) return;
    const outside = (event: PointerEvent) => {
      if (!root.current?.contains(event.target as Node)) setOpen(false);
    };
    document.addEventListener("pointerdown", outside);
    root.current
      ?.querySelector<HTMLButtonElement>('[role="menuitem"]:not(:disabled)')
      ?.focus();
    return () => document.removeEventListener("pointerdown", outside);
  }, [open]);
  const close = () => {
    setOpen(false);
    trigger.current?.focus();
  };
  return (
    <div
      className="menu"
      ref={root}
      onKeyDown={(event) => {
        if (!open) return;
        if (event.key === "Escape") {
          event.preventDefault();
          event.stopPropagation();
          close();
        }
        if (event.key === "ArrowDown" || event.key === "ArrowUp") {
          event.preventDefault();
          const buttons = [
            ...(root.current?.querySelectorAll<HTMLButtonElement>(
              '[role="menuitem"]:not(:disabled)',
            ) ?? []),
          ];
          const index = buttons.indexOf(
            document.activeElement as HTMLButtonElement,
          );
          const step = event.key === "ArrowDown" ? 1 : -1;
          buttons[(index + step + buttons.length) % buttons.length]?.focus();
        }
      }}
    >
      <Button
        ref={trigger}
        size="icon"
        variant="ghost"
        aria-label={label}
        aria-haspopup="menu"
        aria-expanded={open}
        disabled={disabled}
        onClick={(event) => {
          // 靠近滚动区域底部时向上展开，避免菜单被裁切。
          const rect = event.currentTarget.getBoundingClientRect();
          const scroller = event.currentTarget.closest(".task-list, .page");
          const bottom = scroller?.getBoundingClientRect().bottom ?? innerHeight;
          setUp(bottom - rect.bottom < 40 + shown.length * 32);
          setOpen(!open);
        }}
      >
        <MoreHorizontal size={16} />
      </Button>
      {open && (
        <div className={`menu-list ${up ? "is-up" : ""}`} role="menu" aria-label={label}>
          {shown.map((item) => (
            <button
              key={item.label}
              role="menuitem"
              className={item.danger ? "danger-text" : ""}
              disabled={item.disabled}
              onClick={() => {
                close();
                item.onSelect();
              }}
            >
              {item.icon}
              {item.label}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
