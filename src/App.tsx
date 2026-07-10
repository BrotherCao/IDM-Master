import { Toolbar } from "./components/Toolbar";
import { DownloadList } from "./components/DownloadList";
import { SidePanel } from "./components/SidePanel";

export default function App() {
  return (
    <div className="h-screen flex flex-col bg-gray-950 text-gray-100">
      <Toolbar />
      <div className="flex-1 flex overflow-hidden">
        <main className="flex-1 overflow-y-auto p-4">
          <DownloadList />
        </main>
        <SidePanel />
      </div>
    </div>
  );
}
