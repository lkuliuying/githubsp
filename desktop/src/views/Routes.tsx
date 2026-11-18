import { useState } from "react";
import { Check, RefreshCw } from "lucide-react";
import { request, useWorkspace } from "@/lib/api";
import { bytes, date } from "@/lib/model";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { useJob } from "@/components/common";

/** 设置 → 网络 → 线路检测：比较 GitHub 直连和加速线路的可用性与速度。 */
export function RouteCheck() {
  const { snapshot, connected } = useWorkspace();
  const [url, setUrl] = useState("");
  const { busy, run } = useJob();
  const reports = [...(snapshot?.diagnostics ?? [])].sort(
    (a, b) =>
      Number(b.available) - Number(a.available) ||
      b.bytesPerSecond - a.bytesPerSecond,
  );
  const diagnosing = !!snapshot?.diagnosing;
  return (
    <div className="route-check">
      <div className="input-row">
        <Input
          id="probe-url"
          aria-label="测试用的附件地址"
          placeholder="可选：粘贴一个附件链接来测速，留空使用默认附件"
          value={url}
          onChange={(e) => setUrl(e.target.value)}
          disabled={busy || diagnosing}
        />
        <Button
          variant="default"
          disabled={!connected || busy || diagnosing}
          onClick={() => run(() => request("diagnose", { url: url.trim() }))}
        >
          <RefreshCw size={15} className={diagnosing ? "spin" : ""} />
          {diagnosing ? "正在检测…" : "开始检测"}
        </Button>
      </div>
      {reports.length > 0 && (
        <div className="route-results">
          <table>
            <thead>
              <tr>
                <th>线路</th>
                <th>速度</th>
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
                        最快
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
      )}
    </div>
  );
}
