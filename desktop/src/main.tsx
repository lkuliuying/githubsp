import ReactDOM from "react-dom/client";
import { App } from "./App";
import { NoticeApp } from "./NoticeApp";
import { ConfirmProvider } from "./components/common";
import "./styles.css";
import { applyAppearance } from "./lib/appearance";
import { applyZoom } from "./lib/zoom";

const isNotice = new URLSearchParams(location.search).has("notice");
// 首次渲染前应用外观和缩放，避免先闪一下默认样式。提醒窗口尺寸固定，不跟随界面缩放。
applyAppearance();
if (!isNotice) applyZoom();

ReactDOM.createRoot(document.getElementById("root")!).render(
  <ConfirmProvider>
    {new URLSearchParams(location.search).has("notice") ? (
      <NoticeApp />
    ) : (
      <App />
    )}
  </ConfirmProvider>,
);
