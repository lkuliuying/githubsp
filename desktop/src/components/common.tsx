import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { AlertCircle, Inbox, LoaderCircle } from "lucide-react";
import { Modal } from "./ui/dialog";
import { Button } from "./ui/button";
import { notify } from "@/lib/api";

type Confirm = (
  title: string,
  description: string,
  label?: string,
) => Promise<boolean>;
const ConfirmContext = createContext<Confirm>(async () => false);
export const useConfirm = () => useContext(ConfirmContext);
export function ConfirmProvider({ children }: { children: ReactNode }) {
  const [dialog, setDialog] = useState<{
    title: string;
    description: string;
    label: string;
    resolve(value: boolean): void;
  } | null>(null);
  const pending = useRef<((v: boolean) => void) | null>(null);
  const close = (value: boolean) => {
    pending.current?.(value);
    pending.current = null;
    setDialog(null);
  };
  const confirm = useCallback<Confirm>(
    (title, description, label = "确认") =>
      new Promise((resolve) => {
        pending.current?.(false);
        pending.current = resolve;
        setDialog({ title, description, label, resolve });
      }),
    [],
  );
  useEffect(() => () => pending.current?.(false), []);
  return (
    <ConfirmContext.Provider value={confirm}>
      {children}
      <Modal
        open={!!dialog}
        onClose={() => close(false)}
        title={dialog?.title ?? ""}
        footer={
          <>
            <Button onClick={() => close(false)}>取消</Button>
            <Button variant="default" onClick={() => close(true)}>
              {dialog?.label}
            </Button>
          </>
        }
      >
        <p className="confirm-copy">{dialog?.description}</p>
      </Modal>
    </ConfirmContext.Provider>
  );
}
export function useJob() {
  const [busy, setBusy] = useState(false);
  const active = useRef(true);
  const locked = useRef(false);
  useEffect(() => {
    active.current = true;
    return () => {
      active.current = false;
    };
  }, []);
  const run = async (fn: () => Promise<unknown>) => {
    if (locked.current) return;
    locked.current = true;
    setBusy(true);
    try {
      await fn();
    } catch (error) {
      if (active.current)
        notify(String(error instanceof Error ? error.message : error), "error");
    } finally {
      locked.current = false;
      if (active.current) setBusy(false);
    }
  };
  return { busy, run, active };
}
export function Empty({
  title,
  description,
  action,
  icon,
}: {
  title: string;
  description: string;
  action?: ReactNode;
  icon?: ReactNode;
}) {
  return (
    <div className="empty">
      <div className="empty-icon">
        {icon ?? <Inbox size={24} strokeWidth={1.5} />}
      </div>
      <h3>{title}</h3>
      <p>{description}</p>
      {action}
    </div>
  );
}
export function ErrorBox({ text }: { text: string }) {
  return (
    <div className="error-box" role="alert">
      <AlertCircle size={16} />
      <span>{text}</span>
    </div>
  );
}
export function Busy({ text = "正在加载…" }: { text?: string }) {
  return (
    <div className="loading">
      <LoaderCircle size={18} className="spin" />
      {text}
    </div>
  );
}
export function Toggle({
  checked,
  onChange,
  title,
  description,
  disabled = false,
}: {
  checked: boolean;
  onChange(value: boolean): void;
  title: string;
  description?: string;
  disabled?: boolean;
}) {
  return (
    <label className="toggle-row">
      <span>
        <strong>{title}</strong>
        {description && <small>{description}</small>}
      </span>
      <input
        type="checkbox"
        role="switch"
        checked={checked}
        onChange={(e) => onChange(e.target.checked)}
        disabled={disabled}
      />
    </label>
  );
}
