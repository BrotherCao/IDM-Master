import { create } from "zustand";

export interface TaskInfo {
  id: string;
  filename: string;
  url: string;
  progress: number;
  speedBps: number;
  downloaded: number;
  total: number;
  state: "pending" | "running" | "paused" | "completed" | "error" | "cancelled";
}

interface DownloadStore {
  tasks: TaskInfo[];
  setTasks: (tasks: TaskInfo[]) => void;
  addTask: (task: TaskInfo) => void;
  upsertTask: (task: TaskInfo) => void;
  updateTask: (id: string, patch: Partial<TaskInfo>) => void;
  removeTask: (id: string) => void;
}

export const useDownloadStore = create<DownloadStore>((set) => ({
  tasks: [],
  setTasks: (tasks) => set({ tasks }),
  addTask: (task) => set((s) => ({ tasks: [...s.tasks, task] })),
  upsertTask: (task) =>
    set((s) => {
      const idx = s.tasks.findIndex((t) => t.id === task.id);
      if (idx >= 0) {
        return { tasks: s.tasks.map((t) => (t.id === task.id ? task : t)) };
      }
      return { tasks: [...s.tasks, task] };
    }),
  updateTask: (id, patch) =>
    set((s) => ({
      tasks: s.tasks.map((t) => (t.id === id ? { ...t, ...patch } : t)),
    })),
  removeTask: (id) =>
    set((s) => ({ tasks: s.tasks.filter((t) => t.id !== id) })),
}));
