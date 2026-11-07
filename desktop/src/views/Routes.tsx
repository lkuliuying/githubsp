import { useState } from "react";
import { Check, Network, RefreshCw } from "lucide-react";
import { request, useWorkspace } from "@/lib/api";
import { bytes, date } from "@/lib/model";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Empty, useJob } from "@/components/common";
import { PageHeader } from "@/components/PageActivity";

export function Routes() {
  const { snapshot, connected } = useWorkspace();
  const [url, setUrl] = useState("");
  const { busy, run } = useJob();
  const reports = [...(snapshot?.diagnostics ?? [])].sort(
    (a, b) =>
      Number(b.available) - Number(a.available) ||
      b.bytesPerSecond - a.bytesPerSecond,
  );
  return (
    <section className="page routes-page" tabIndex={-1}>
      <PageHeader>
        <h1>线路检测</h1>
      </PageHeader>
      <div className="route-input">
        <label className="field-label" htmlFor="probe-url">
          测试附件地址 <span className="muted">可选</span>
        </label>
        <div className="input-row">
          <Input
            id="probe-url"
            placeholder="留空使用默认测试附件"
            value={url}
            onChange={(e) => setUrl(e.target.value)}
            disabled={busy || snapshot?.diagnosing}
          />
          <Button
            variant="default"
            disabled={!connected || busy || snapshot?.diagnosing}
            onClick={() => run(() => request("diagnose", { url: url.trim() }))}
          >
            <RefreshCw
              size={15}
              className={snapshot?.diagnosing ? "spin" : ""}
            />
            {snapshot?.diagnosing ? "正在检测…" : "开始检测"}
          </Button>
        </div>
        <p className="form-note">
          每条线路最多读取约 524 KB，最长检测 10
          秒。结果代表当前附件与网络情况。
        </p>
      </div>
      {reports.length ? (
        <div className="route-results">
          <table>
            <thead>
              <tr>
                <th>下载线路</th>
                <th>传输速度</th>
                <th>状态</th>
                <th>检测时间</th>
              </tr>
            </thead>
            <tbody>
              {reports.map((report, index) => (
                <tr key={report.id}>
                  <td>
                    <strong>{report.name}</strong>
                    {index === 0 && report.available && (
                      <span className="status status-completed">
                        <Check size={12} />
                        当前最快
                      </span>
                    )}
                  </td>
                  <td className="numeric">
                    {report.available
                      ? `${bytes(report.bytesPerSecond)}/s`
                      : "—"}
                  </td>
                  <td>
                    <span
                      className={`status ${report.available ? "status-completed" : "status-failed"}`}
                    >
                      {report.available ? "可用" : "不可用"}
                    </span>
                    {!report.available && report.error && (
                      <small>{report.error}</small>
                    )}
                  </td>
                  <td className="muted">{date(report.checkedAt)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      ) : (
        <Empty
          title={snapshot?.diagnosing ? "正在检测线路" : "尚无检测结果"}
          description="检测 GitHub 直连及公共加速线路的可用性与当前速度。"
          icon={<Network size={24} />}
        />
      )}
      <p className="route-disclosure">
        使用公共加速线路时，服务会收到公开附件 URL。检测与下载不会执行附件文件。
      </p>
    </section>
  );
}
