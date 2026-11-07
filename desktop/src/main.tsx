import ReactDOM from "react-dom/client";
import { App } from "./App";
import { NoticeApp } from "./NoticeApp";
import { ConfirmProvider } from "./components/common";
import "./styles.css";

ReactDOM.createRoot(document.getElementById("root")!).render(
  <ConfirmProvider>
    {new URLSearchParams(location.search).has("notice") ? (
      <NoticeApp />
    ) : (
      <App />
    )}
  </ConfirmProvider>,
);
