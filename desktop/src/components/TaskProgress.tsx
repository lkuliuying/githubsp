// 改编自 Motrix task-progress.tsx（MIT）；下载状态及进度计算接入 GitHubSP。
import { bytes, progress, running, type Task } from "@/lib/model";
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
      >
        <span style={{ width: `${pct ?? 0}%` }} />
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
