// 改编自 Motrix task-progress.tsx（MIT）；下载状态及进度计算接入 GitHubSP。
// 用 scaleX 绘制进度、不确定状态用渐变滑条，参考 cult-ui halo-progress（MIT），见 THIRD_PARTY_NOTICES.md。
import type { CSSProperties } from "react";
import { bytes, progress, running, type Task, type TaskStatus } from "@/lib/model";

// 这些状态下进度数字不动，但程序在忙（检测线路、等网络、重试、校验），叠一层滑动光带表示「在处理」。
const busyStatuses: TaskStatus[] = ["probing", "waiting_network", "retrying", "verifying"];
export function TaskProgress({ task }: { task: Task }) {
  const pct = progress(task);
  const label = pct === null ? "大小未知" : `${pct.toFixed(1)}%`;
  return (
    <div className={`task-progress tone-${task.status}`}>
      <div
        className="progress-track"
        role="progressbar"
        aria-label={`${task.filename} 下载进度`}
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={pct ?? undefined}
        aria-valuetext={
          pct === null ? `已下载 ${bytes(task.downloaded)}，总大小未知` : label
        }
        data-indeterminate={(pct === null && running(task.status)) || undefined}
        data-busy={(pct !== null && busyStatuses.includes(task.status)) || undefined}
      >
        <span
          style={
            { "--progress-scale": Math.min(1, (pct ?? 0) / 100) } as CSSProperties
          }
        />
      </div>
      <div className="progress-caption">
        <span>
          {pct === null
            ? `已下载 ${bytes(task.downloaded)}`
            : `${bytes(task.downloaded)} / ${bytes(task.total)}`}
        </span>
        <span>{label}</span>
      </div>
    </div>
  );
}
