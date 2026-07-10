import { useState } from "react";
import { addDownload } from "../api";

export function Toolbar() {
  const [showDialog, setShowDialog] = useState(false);
  const [url, setUrl] = useState("");
  const [saveDir, setSaveDir] = useState(".");

  const handleAdd = async () => {
    if (!url.trim()) return;
    try {
      await addDownload(url, saveDir);
      setUrl("");
      setShowDialog(false);
    } catch (e) {
      alert(`Failed: ${e}`);
    }
  };

  return (
    <>
      <div className="flex items-center gap-3 px-4 py-2 bg-gray-900 border-b border-gray-800" data-tauri-drag-region>
        <h1 className="text-lg font-bold text-blue-400 mr-4">IDM Master</h1>
        <button
          className="px-3 py-1.5 bg-blue-600 hover:bg-blue-700 rounded text-sm font-medium transition-colors"
          onClick={() => setShowDialog(true)}
        >
          + 新建下载
        </button>
        <div className="flex-1" />
        <button className="px-3 py-1.5 bg-gray-700 hover:bg-gray-600 rounded text-sm transition-colors">
          ⚙ 设置
        </button>
      </div>

      {/* Add Download Dialog */}
      {showDialog && (
        <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-50" onClick={() => setShowDialog(false)}>
          <div className="bg-gray-800 rounded-lg p-6 w-[480px] space-y-4" onClick={(e) => e.stopPropagation()}>
            <h2 className="text-lg font-semibold">新建下载</h2>
            <div>
              <label className="block text-sm text-gray-400 mb-1">URL</label>
              <input
                className="w-full px-3 py-2 bg-gray-700 rounded border border-gray-600 focus:border-blue-500 outline-none text-sm"
                placeholder="https://example.com/file.zip"
                value={url}
                onChange={(e) => setUrl(e.target.value)}
                onKeyDown={(e) => e.key === "Enter" && handleAdd()}
                autoFocus
              />
            </div>
            <div>
              <label className="block text-sm text-gray-400 mb-1">保存目录</label>
              <input
                className="w-full px-3 py-2 bg-gray-700 rounded border border-gray-600 focus:border-blue-500 outline-none text-sm"
                value={saveDir}
                onChange={(e) => setSaveDir(e.target.value)}
              />
            </div>
            <div className="flex justify-end gap-2 pt-2">
              <button className="px-4 py-2 bg-gray-600 hover:bg-gray-500 rounded text-sm" onClick={() => setShowDialog(false)}>
                取消
              </button>
              <button className="px-4 py-2 bg-blue-600 hover:bg-blue-700 rounded text-sm font-medium" onClick={handleAdd}>
                开始下载
              </button>
            </div>
          </div>
        </div>
      )}
    </>
  );
}
