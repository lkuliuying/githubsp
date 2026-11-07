// 改编自 cc-switch 的 Radix Dialog 封装（MIT），使用本项目的布局与关闭规则。
import * as Dialog from "@radix-ui/react-dialog";
import { X } from "lucide-react";
import { useEffect, type ReactNode } from "react";
import { blockNavigation } from "@/lib/api";
import { usePageActive } from "../PageActivity";
import { Button } from "./button";

export function Modal({
  open,
  onClose,
  title,
  description,
  children,
  footer,
  wide = false,
  busy = false,
}: {
  open: boolean;
  onClose(): void;
  title: string;
  description?: string;
  children: ReactNode;
  footer?: ReactNode;
  wide?: boolean;
  busy?: boolean;
}) {
  const pageActive = usePageActive();
  const shown = open && pageActive;
  useEffect(() => {
    if (shown) return blockNavigation();
  }, [shown]);
  return (
    <Dialog.Root
      open={shown}
      onOpenChange={(next) => {
        if (!next && !busy) onClose();
      }}
    >
      <Dialog.Portal>
        <Dialog.Overlay className="modal-overlay" />
        <Dialog.Content
          className={`modal ${wide ? "modal-wide" : ""}`}
          onInteractOutside={(event) => event.preventDefault()}
          onEscapeKeyDown={(event) => {
            if (busy) event.preventDefault();
          }}
        >
          <header className="modal-header">
            <div>
              <Dialog.Title>{title}</Dialog.Title>
              <Dialog.Description
                className={description ? "modal-description" : "sr-only"}
              >
                {description || title}
              </Dialog.Description>
            </div>
            <Button
              size="icon"
              variant="ghost"
              aria-label="关闭对话框"
              onClick={onClose}
              disabled={busy}
            >
              <X size={18} />
            </Button>
          </header>
          <div className="modal-body">{children}</div>
          {footer && <footer className="modal-footer">{footer}</footer>}
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
