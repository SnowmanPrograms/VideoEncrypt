import { useTaskProgress } from "@/hooks/useTaskProgress";
import { useDragDrop } from "@/hooks/useDragDrop";
import { useAppStore } from "@/stores/appStore";
import { useI18n } from "@/stores/i18nStore";
import { ThemeProvider } from "@/components/ThemeProvider";
import { FileList } from "@/components/FileList";
import { TaskConfigPanel } from "@/components/TaskConfigPanel";
import { Toast } from "@/components/Toast";
import { LanguageSwitcher } from "@/components/LanguageSwitcher";
import { ThemeSwitcher } from "@/components/ThemeSwitcher";
import { Button } from "@/components/ui/button";
import { Progress } from "@/components/ui/progress";
import { Separator } from "@/components/ui/separator";
import { selectFiles, selectFolder, cancelTask } from "@/lib/tauri";
import { formatBytes } from "@/lib/utils";
import { Lock, Unlock, FileUp, FolderOpen, X, Upload } from "lucide-react";

function AppContent() {
  useTaskProgress();
  const i18n = useI18n((s) => s.t);
  const {
    addFiles,
    taskMode,
    setTaskMode,
    isProcessing,
    progress,
    currentTask,
    files,
  } = useAppStore();

  const { isDragOver, dragProps } = useDragDrop();

  const handleSelectFiles = async () => {
    try {
      const selectedFiles = await selectFiles(false);
      addFiles(selectedFiles);
    } catch (e) {
      console.error("Failed to select files:", e);
    }
  };

  const handleSelectFolder = async () => {
    try {
      const selectedFiles = await selectFolder(true);
      addFiles(selectedFiles);
    } catch (e) {
      console.error("Failed to select folder:", e);
    }
  };

  const handleCancel = async () => {
    if (currentTask?.id) {
      await cancelTask(currentTask.id);
    }
  };

  const progressPercent =
    progress?.total_bytes && progress.total_bytes > 0
      ? (progress.processed_bytes / progress.total_bytes) * 100
      : 0;

  const totalSize = files.reduce((sum, file) => sum + file.size, 0);

  return (
    <div className="flex flex-col h-screen">
      <Toast />
      <header className="border-b bg-card">
        {/* Row 1: Logo + Title + Settings */}
        <div className="flex items-center justify-between px-4 py-2 border-b">
          <div className="flex items-center gap-2">
            <Lock className="h-5 w-5 text-primary" />
            <h1 className="text-base font-bold">{i18n.app.title}</h1>
            <span className="text-xs text-muted-foreground ml-1">
              {i18n.app.subtitle}
            </span>
          </div>
          <div className="flex items-center gap-2">
            <LanguageSwitcher />
            <ThemeSwitcher />
          </div>
        </div>

        {/* Row 2: Toolbar */}
        <div className="flex items-center gap-2 px-4 py-2">
          <Button
            variant="outline"
            size="sm"
            onClick={handleSelectFiles}
            disabled={isProcessing}
          >
            <FileUp className="h-3.5 w-3.5 mr-1.5" />
            {i18n.button.selectFiles}
          </Button>
          <Button
            variant="outline"
            size="sm"
            onClick={handleSelectFolder}
            disabled={isProcessing}
          >
            <FolderOpen className="h-3.5 w-3.5 mr-1.5" />
            {i18n.button.selectFolder}
          </Button>

          <Separator orientation="vertical" className="h-6 mx-1" />

          <div className="flex gap-1">
            <Button
              variant={taskMode === "Encrypt" ? "default" : "outline"}
              size="sm"
              onClick={() => setTaskMode("Encrypt")}
              disabled={isProcessing}
            >
              <Lock className="h-3.5 w-3.5 mr-1.5" />
              {i18n.config.encrypt}
            </Button>
            <Button
              variant={taskMode === "Decrypt" ? "default" : "outline"}
              size="sm"
              onClick={() => setTaskMode("Decrypt")}
              disabled={isProcessing}
            >
              <Unlock className="h-3.5 w-3.5 mr-1.5" />
              {i18n.config.decrypt}
            </Button>
          </div>
        </div>
      </header>

      <div className="flex flex-1 overflow-hidden">
        {/* Main Content Area */}
        <main
          className="flex-1 flex flex-col px-4 py-3 overflow-auto relative"
          {...dragProps}
        >
          {isDragOver && files.length > 0 && (
            <div className="absolute inset-0 z-20 flex items-center justify-center bg-background/80 backdrop-blur-sm pointer-events-none">
              <div className="flex flex-col items-center gap-3 p-6 rounded-xl border-2 border-dashed border-primary bg-card shadow-lg">
                <Upload className="h-10 w-10 text-primary animate-bounce" />
                <p className="text-sm font-medium text-primary">
                  {i18n.file.dropActive}
                </p>
              </div>
            </div>
          )}
          <div className="flex-1">
            <FileList />
          </div>
        </main>

        {/* Sidebar: Configuration Panel */}
        <aside className="w-72 border-l bg-muted/30 p-3 overflow-y-auto">
          <h2 className="text-sm font-semibold mb-3">{i18n.config.title}</h2>
          <TaskConfigPanel />
        </aside>
      </div>

      <footer className="border-t bg-card py-2 px-4">
        <div className="flex items-center justify-between text-xs">
          <div className="flex items-center gap-4">
            <span>{i18n.status.files}: {files.length}</span>
            <span>{i18n.status.totalSize}: {formatBytes(totalSize)}</span>
            {isProcessing && progress && (
              <>
                <Separator orientation="vertical" className="h-4" />
                <span className="text-primary font-medium">
                  {progress.phase}
                </span>
                {progress.current_file && (
                  <span className="text-muted-foreground truncate max-w-xs">
                    {progress.current_file}
                  </span>
                )}
              </>
            )}
          </div>

          <div className="flex items-center gap-3">
            {isProcessing && progress && (
              <>
                <div className="flex items-center gap-2">
                  <span className="text-muted-foreground">
                    {formatBytes(progress.processed_bytes)} / {formatBytes(progress.total_bytes)}
                  </span>
                  <span className="font-medium">{progressPercent.toFixed(1)}%</span>
                </div>
                <Progress value={progressPercent} className="w-32 h-2" />
                {progress.stats && (
                  <span className="text-muted-foreground">
                    {progress.stats.throughput_mbps.toFixed(2)} MB/s
                  </span>
                )}
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={handleCancel}
                  className="h-6 px-2"
                >
                  <X className="h-3 w-3" />
                </Button>
              </>
            )}
            {!isProcessing && (
              <span className="text-muted-foreground">{i18n.status.ready}</span>
            )}
          </div>
        </div>
      </footer>
    </div>
  );
}

function App() {
  return (
    <ThemeProvider>
      <AppContent />
    </ThemeProvider>
  );
}

export default App;
