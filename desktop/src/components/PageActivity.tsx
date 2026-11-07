import {
  createContext,
  useContext,
  useLayoutEffect,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { createPortal } from "react-dom";

export const PageActivity = createContext(true);
export const usePageActive = () => useContext(PageActivity);
export function usePageActiveRef() {
  const visible = usePageActive();
  const ref = useRef(visible);
  ref.current = visible;
  return ref;
}

export function PageHeader({ children }: { children: ReactNode }) {
  const visible = usePageActive();
  const [slot, setSlot] = useState<HTMLElement | null>(null);
  useLayoutEffect(() => {
    setSlot(document.getElementById("page-header-slot"));
  }, []);
  return visible && slot
    ? createPortal(<header className="page-header">{children}</header>, slot)
    : null;
}
