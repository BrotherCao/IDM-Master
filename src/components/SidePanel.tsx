export function SidePanel() {
  return (
    <aside className="w-56 p-4 bg-gray-900 border-l border-gray-800 text-sm space-y-4">
      <div>
        <div className="text-gray-500 text-xs uppercase tracking-wide">总计速度</div>
        <div className="text-lg font-mono text-green-400">0 B/s</div>
      </div>
      <div>
        <div className="text-gray-500 text-xs uppercase tracking-wide">今日下载</div>
        <div className="text-lg font-mono">0 B</div>
      </div>
      <div>
        <div className="text-gray-500 text-xs uppercase tracking-wide">队列等待</div>
        <div className="text-lg font-mono">0</div>
      </div>
    </aside>
  );
}
