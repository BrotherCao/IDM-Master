export function DownloadList() {
  return (
    <div className="space-y-4">
      <section>
        <h2 className="text-sm font-semibold text-gray-400 mb-2">正在下载</h2>
        <div className="text-gray-500 text-sm p-8 text-center border border-dashed border-gray-700 rounded-lg">
          暂无下载任务。点击「+ 新建下载」开始。
        </div>
      </section>
      <section>
        <h2 className="text-sm font-semibold text-gray-400 mb-2">已完成</h2>
        <div className="text-gray-500 text-sm p-8 text-center border border-dashed border-gray-700 rounded-lg">
          暂无已完成任务。
        </div>
      </section>
    </div>
  );
}
