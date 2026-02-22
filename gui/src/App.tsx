import { useEffect } from "react";
import { useTaskProgress } from "@/hooks/useTaskProgress";
import { useAppStore } from "@/stores/appStore";
import { FileList } from "@/components/FileList";
import { TaskConfigPanel } from "@/components/TaskConfigPanel";
import { ProgressPanel } from "@/components/ProgressPanel";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Lock } from "lucide-react";

function AppContent() {
  useTaskProgress();
  const isProcessing = useAppStore((state) => state.isProcessing);

  return (
    <div className="flex flex-col h-screen">
      <header className="border-b bg-card">
        <div className="container flex items-center gap-2 py-4">
          <Lock className="h-6 w-6 text-primary" />
          <h1 className="text-xl font-bold">Media Lock</h1>
          <span className="text-xs text-muted-foreground ml-2">
            Video Encryption Tool
          </span>
        </div>
      </header>

      <main className="flex-1 container py-6 overflow-auto">
        <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
          <div className="space-y-6">
            <Card>
              <CardHeader>
                <CardTitle>Files</CardTitle>
              </CardHeader>
              <CardContent>
                <FileList />
              </CardContent>
            </Card>

            <ProgressPanel />
          </div>

          <div>
            <Card>
              <CardHeader>
                <CardTitle>Configuration</CardTitle>
              </CardHeader>
              <CardContent>
                <TaskConfigPanel />
              </CardContent>
            </Card>
          </div>
        </div>
      </main>

      <footer className="border-t bg-card py-3">
        <div className="container text-center text-sm text-muted-foreground">
          Media Lock v0.1.0 - In-place video encryption with AES-256-CTR
        </div>
      </footer>
    </div>
  );
}

function App() {
  return <AppContent />;
}

export default App;
