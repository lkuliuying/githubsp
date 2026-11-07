import {
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type ComponentType,
} from "react";
import type { View } from "@/lib/api";
import { PageActivity } from "./PageActivity";

export function PageHost({
  view,
  pages,
}: {
  view: View;
  pages: Record<View, ComponentType>;
}) {
  const [visited, setVisited] = useState<View[]>([view]);
  const host = useRef<HTMLDivElement>(null);
  const shown = visited.includes(view) ? visited : [...visited, view];
  useEffect(() => {
    setVisited((old) => (old.includes(view) ? old : [...old, view]));
  }, [view]);
  useLayoutEffect(() => {
    const focused = document.activeElement;
    if (
      focused instanceof HTMLElement &&
      focused.closest(".page-frame[hidden]")
    ) {
      host.current
        ?.querySelector<HTMLElement>(".page-frame:not([hidden]) .page")
        ?.focus({ preventScroll: true });
    }
  }, [view]);
  return (
    <div className="page-container" ref={host}>
      {shown.map((key) => {
        const Page = pages[key];
        const active = key === view;
        return (
          <div
            key={key}
            className="page-frame"
            data-page={key}
            hidden={!active}
            aria-hidden={!active}
            ref={(element) => {
              if (element) element.inert = !active;
            }}
          >
            <PageActivity.Provider value={active}>
              <Page />
            </PageActivity.Provider>
          </div>
        );
      })}
    </div>
  );
}
