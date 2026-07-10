import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useDownloadStore, TaskInfo } from "../stores/useDownloadStore";

export interface TaskInfoPayload {
  id: string;
  filename: string;
  url: string;
  state: string;
  progress: number;
  speed_bps: number;
  downloaded: number;
  total: number;
}

/** 调用 Rust 命令添加下载 */
export async function addDownload(url: string, saveDir: string): Promise<string> {
  return invoke("add_download", { url, saveDir });
}

/** 暂停任务 */
export async function pauseTask(id: string): Promise<void> {
  return invoke("pause_task", { id });
}

/** 恢复任务 */
export async function resumeTask(id: string): Promise<void> {
  return invoke("resume_task", { id });
}

/** 取消任务 */
export async function cancelTask(id: string): Promise<void> {
  return invoke("cancel_task", { id });
}

/** 获取任务列表 */
export async function listTasks(): Promise<TaskInfoPayload[]> {
  return invoke("list_tasks");
}

/** 设置分类开关 */
export async function setClassify(enabled: boolean): Promise<void> {
  return invoke("set_classify", { enabled });
}

/** 获取分类状态 */
export async function getClassify(): Promise<boolean> {
  return invoke("get_classify");
}

/** 监听下载进度事件并更新 store */
export async function listenToProgress() {
  await listen<TaskInfoPayload>("download-progress", (event) => {
    const payload = event.payload;
    const task: TaskInfo = {
      id: payload.id,
      filename: payload.filename,
      url: payload.url,
      state: payload.state as TaskInfo["state"],
      progress: payload.progress,
      speedBps: payload.speed_bps,
      downloaded: payload.downloaded,
      total: payload.total,
    };
    useDownloadStore.getState().upsertTask(task);
  });
}
