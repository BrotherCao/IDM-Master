import { useDownloadStore } from "../stores/useDownloadStore";
import { DownloadItem } from "./DownloadItem";

function formatBytes(bytes: number): string {
  if (bytes >= 1_073_741_824) return `${(bytes / 1_073_741_824).toFixed(2)} GB`;
  if (bytes >= 1_048_576) return `${(bytes / 1_048_576).toFixed(2)} MB`;
  if (bytes >= 1024) return `${(bytes / 1024).toFixed(2)} KB`;
  return `${bytes} B`;
}

function formatSpeed(bps: number): string {
  if (bps >= 1_073_741_824) return `${(bps / 1_073_741_824).toFixed(1)} GB/s`;
  if (bps >= 1_048_576) return `${(bps / 1_048_576).toFixed(1)} MB/s`;
  if (bps >= 1024) return `${(bps / 1024).toFixed(1)} KB/s`;
  return bps > 0 ? `${bps.toFixed(0)} B/s` : "0 B/s";
}

export function DownloadList() {
  const tasks = useDownloadStore((s) => s.tasks);

  const active = tasks.filter((t) => ["running", "pending", "paused"].includes(t.state));
  const completed = tasks.filter((t) => t.state === "completed");
  const errors = tasks.filter((t) => t.state === "error");

  if (tasks.length === 0) {
    return (
      <div className="text-gray-500 text-sm p-8 text-center border border-dashed border-gray-700 rounded-lg mt-4">
        暂无下载任务。点击「+ 新建下载」开始。
      </div>
    );
  }

  return (
    <div className="space-y-4">
      {active.length > 0 && (
        <section>
          <h2 className="text-sm font-semibold text-gray-400 mb-2">
            正在下载 ({active.length})
          </h2>
          <div className="space-y-2">
            {active.map((t) => (
              <DownloadItem key={t.id} task={t} formatBytes={formatBytes} formatSpeed={formatSpeed} />
            ))}
          </div>
        </section>
      )}

      {completed.length > 0 && (
        <section>
          <h2 className="text-sm font-semibold text-gray-400 mb-2">
            已完成 ({completed.length})
          </h2>
          <div className="space-y-1 text-sm text-gray-500">
            {completed.map((t) => (
              <div key={t.id} className="flex justify-between px-3 py-1 bg-gray-800/50 rounded">
                <span>{t.filename}</span>
                <span>{formatBytes(t.total)}</span>
              </div>
            ))}
          </div>
        </section>
      )}

      {errors.length > 0 && (
        <section>
          <h2 className="text-sm font-semibold text-red-400 mb-2">错误</h2>
          {errors.map((t) => (
            <div key={t.id} className="text-red-400 text-sm px-3 py-1">{t.filename}</div>
          ))}
        </section>
      )}
    </div>
  );
}
