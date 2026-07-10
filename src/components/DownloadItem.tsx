import { TaskInfo } from "../stores/useDownloadStore";
import { pauseTask, resumeTask, cancelTask } from "../api";

interface Props {
  task: TaskInfo;
  formatBytes: (b: number) => string;
  formatSpeed: (b: number) => string;
}

export function DownloadItem({ task, formatBytes, formatSpeed }: Props) {
  const pct = (task.progress * 100).toFixed(1);
  const barColor =
    task.state === "paused" ? "bg-yellow-500" :
    task.state === "error" ? "bg-red-500" : "bg-blue-500";

  return (
    <div className="bg-gray-800 rounded-lg p-3 space-y-2">
      <div className="flex items-center justify-between">
        <span className="text-sm font-medium truncate flex-1 mr-2" title={task.filename}>
          {task.filename}
        </span>
        <span className="text-xs text-gray-400">{pct}%</span>
      </div>

      {/* Progress bar */}
      <div className="h-2 bg-gray-700 rounded-full overflow-hidden">
        <div
          className={`h-full ${barColor} transition-all duration-300 rounded-full`}
          style={{ width: `${task.progress * 100}%` }}
        />
      </div>

      <div className="flex items-center justify-between text-xs text-gray-400">
        <span>{formatSpeed(task.speedBps)}</span>
        <span>{formatBytes(task.downloaded)} / {formatBytes(task.total)}</span>
      </div>

      {/* Action buttons */}
      <div className="flex gap-1">
        {task.state === "running" && (
          <button
            className="px-2 py-0.5 bg-yellow-700 hover:bg-yellow-600 rounded text-xs"
            onClick={() => pauseTask(task.id)}
          >
            暂停
          </button>
        )}
        {task.state === "paused" && (
          <button
            className="px-2 py-0.5 bg-green-700 hover:bg-green-600 rounded text-xs"
            onClick={() => resumeTask(task.id)}
          >
            继续
          </button>
        )}
        {(task.state === "running" || task.state === "paused") && (
          <button
            className="px-2 py-0.5 bg-red-700 hover:bg-red-600 rounded text-xs"
            onClick={() => cancelTask(task.id)}
          >
            取消
          </button>
        )}
      </div>
    </div>
  );
}
