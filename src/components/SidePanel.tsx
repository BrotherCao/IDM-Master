import { useDownloadStore } from "../stores/useDownloadStore";

export function SidePanel() {
  const tasks = useDownloadStore((s) => s.tasks);

  const active = tasks.filter((t) => ["running", "pending"].includes(t.state));
  const totalSpeed = active.reduce((sum, t) => sum + t.speedBps, 0);
  const todayTotal = tasks
    .filter((t) => t.state === "completed")
    .reduce((sum, t) => sum + t.total, 0);
  const queued = tasks.filter((t) => t.state === "pending").length;

  const formatBps = (bps: number) => {
    if (bps >= 1_073_741_824) return `${(bps / 1_073_741_824).toFixed(1)} GB/s`;
    if (bps >= 1_048_576) return `${(bps / 1_048_576).toFixed(1)} MB/s`;
    if (bps >= 1024) return `${(bps / 1024).toFixed(1)} KB/s`;
    return bps > 0 ? `${bps.toFixed(0)} B/s` : "0 B/s";
  };

  const formatBytes = (b: number) => {
    if (b >= 1_073_741_824) return `${(b / 1_073_741_824).toFixed(2)} GB`;
    if (b >= 1_048_576) return `${(b / 1_048_576).toFixed(2)} MB`;
    if (b >= 1024) return `${(b / 1024).toFixed(2)} KB`;
    return `${b} B`;
  };

  return (
    <aside className="w-52 p-4 bg-gray-900 border-l border-gray-800 text-sm space-y-5">
      <div>
        <div className="text-gray-500 text-xs uppercase tracking-wide mb-0.5">总计速度</div>
        <div className="text-lg font-mono text-green-400">{formatBps(totalSpeed)}</div>
      </div>
      <div>
        <div className="text-gray-500 text-xs uppercase tracking-wide mb-0.5">今日下载</div>
        <div className="text-lg font-mono">{formatBytes(todayTotal)}</div>
      </div>
      <div>
        <div className="text-gray-500 text-xs uppercase tracking-wide mb-0.5">队列等待</div>
        <div className="text-lg font-mono">{queued}</div>
      </div>
      <div>
        <div className="text-gray-500 text-xs uppercase tracking-wide mb-0.5">总任务数</div>
        <div className="text-lg font-mono">{tasks.length}</div>
      </div>
    </aside>
  );
}
