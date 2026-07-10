import { useState } from "react";
import { addDownload, setClassify, getClassify } from "../api";

export function Toolbar() {
  const [showDialog, setShowDialog] = useState(false);
  const [showSettings, setShowSettings] = useState(false);
  const [url, setUrl] = useState("");
  const [saveDir, setSaveDir] = useState("");
  const [classifyEnabled, setClassifyEnabled] = useState(true);

  // 初始化保存目录为 Downloads
  if (!saveDir) {
    import("@tauri-apps/plugin-dialog").then(({ defaultDownloadDir }) => {
      defaultDownloadDir?.().then(d => { if (d) setSaveDir(d); });
    }).catch(() => setSaveDir("."));
  }

  // ── 文件夹选择器 ──
  const pickFolder = async () => {
    try {
      const { open } = await import("@tauri-apps/plugin-dialog");
      const selected = await open({ directory: true, multiple: false, title: "选择保存目录" });
      if (selected) setSaveDir(selected as string);
    } catch {
      // 回退到手动输入
    }
  };

  // ── 添加下载 ──
  const handleAdd = async () => {
    if (!url.trim()) return;
    try {
      await addDownload(url, saveDir || ".");
      setUrl("");
      setShowDialog(false);
    } catch (e) {
      alert(`Failed: ${e}`);
    }
  };

  // ── 设置面板 ──
  const loadSettings = async () => {
    try {
      const enabled = await getClassify();
      setClassifyEnabled(enabled);
    } catch { /* ignore */ }
  };

  const toggleClassify = async () => {
    const next = !classifyEnabled;
    setClassifyEnabled(next);
    try { await setClassify(next); } catch { /* ignore */ }
  };

  return (
    <>
      {/* 工具栏 */}
      <div className="flex items-center gap-3 px-4 py-2 bg-gray-900 border-b border-gray-800" data-tauri-drag-region>
        <h1 className="text-lg font-bold text-blue-400 mr-4">IDM Master</h1>
        <button
          className="px-3 py-1.5 bg-blue-600 hover:bg-blue-700 rounded text-sm font-medium transition-colors"
          onClick={() => setShowDialog(true)}
        >
          + 新建下载
        </button>
        <div className="flex-1" />
        <button
          className="px-3 py-1.5 bg-gray-700 hover:bg-gray-600 rounded text-sm transition-colors"
          onClick={() => { setShowSettings(true); loadSettings(); }}
        >
          ⚙ 设置
        </button>
      </div>

      {/* 新建下载对话框 */}
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
              <div className="flex gap-2">
                <input
                  className="flex-1 px-3 py-2 bg-gray-700 rounded border border-gray-600 focus:border-blue-500 outline-none text-sm"
                  value={saveDir}
                  onChange={(e) => setSaveDir(e.target.value)}
                  placeholder="选择或输入保存目录..."
                />
                <button
                  className="px-3 py-2 bg-gray-600 hover:bg-gray-500 rounded text-sm transition-colors flex items-center justify-center"
                  onClick={pickFolder}
                  title="浏览文件夹"
                >
                  <svg xmlns="http://www.w3.org/2000/svg" className="h-5 w-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
                    <path strokeLinecap="round" strokeLinejoin="round" d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z" />
                  </svg>
                </button>
              </div>
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

      {/* 设置面板 */}
      {showSettings && (
        <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-50" onClick={() => setShowSettings(false)}>
          <div className="bg-gray-800 rounded-lg p-6 w-[480px] space-y-5" onClick={(e) => e.stopPropagation()}>
            <h2 className="text-lg font-semibold">设置</h2>

            {/* 下载分类开关 */}
            <div className="flex items-center justify-between py-2">
              <div>
                <div className="text-sm font-medium">下载分类</div>
                <div className="text-xs text-gray-400 mt-0.5">按文件类型自动分到视频/音频/文档等子目录</div>
              </div>
              <button
                onClick={toggleClassify}
                className={`relative w-11 h-6 rounded-full transition-colors ${classifyEnabled ? "bg-blue-600" : "bg-gray-600"}`}
              >
                <span className={`absolute top-0.5 left-0.5 w-5 h-5 bg-white rounded-full transition-transform ${classifyEnabled ? "translate-x-5" : "translate-x-0"}`} />
              </button>
            </div>

            {/* 默认保存目录 */}
            <div>
              <label className="block text-sm text-gray-400 mb-1">默认保存目录</label>
              <div className="flex gap-2">
                <input
                  className="flex-1 px-3 py-2 bg-gray-700 rounded border border-gray-600 focus:border-blue-500 outline-none text-sm"
                  value={saveDir}
                  onChange={(e) => setSaveDir(e.target.value)}
                  placeholder="默认下载目录..."
                />
                <button
                  className="px-3 py-2 bg-gray-600 hover:bg-gray-500 rounded text-sm transition-colors flex items-center"
                  onClick={pickFolder}
                  title="浏览文件夹"
                >
                  <svg xmlns="http://www.w3.org/2000/svg" className="h-5 w-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
                    <path strokeLinecap="round" strokeLinejoin="round" d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z" />
                  </svg>
                </button>
              </div>
            </div>

            {/* 关于 */}
            <div className="pt-2 border-t border-gray-700 text-xs text-gray-500 space-y-1">
              <p>IDM Master v0.1.0</p>
              <p>基于 Rust + Tauri v2 + React 构建</p>
              <p>引擎模块: 连接池 · Direct I/O · 站点规则 · 下载分类</p>
            </div>

            <div className="flex justify-end pt-1">
              <button className="px-4 py-2 bg-blue-600 hover:bg-blue-700 rounded text-sm font-medium" onClick={() => setShowSettings(false)}>
                关闭
              </button>
            </div>
          </div>
        </div>
      )}
    </>
  );
}
