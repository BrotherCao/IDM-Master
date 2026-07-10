export function Toolbar() {
  return (
    <div className="flex items-center gap-3 px-4 py-2 bg-gray-900 border-b border-gray-800" data-tauri-drag-region>
      <h1 className="text-lg font-bold text-blue-400 mr-4">IDM Master</h1>
      <button className="px-3 py-1.5 bg-blue-600 hover:bg-blue-700 rounded text-sm font-medium transition-colors">
        + 新建下载
      </button>
      <button className="px-3 py-1.5 bg-gray-700 hover:bg-gray-600 rounded text-sm transition-colors">
        ▶ 全部开始
      </button>
      <button className="px-3 py-1.5 bg-gray-700 hover:bg-gray-600 rounded text-sm transition-colors">
        ⏸ 全部暂停
      </button>
      <div className="flex-1" />
      <button className="px-3 py-1.5 bg-gray-700 hover:bg-gray-600 rounded text-sm transition-colors">
        ⚙ 设置
      </button>
    </div>
  );
}
