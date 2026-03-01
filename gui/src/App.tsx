import { useTaskProgress } from "@/hooks/useTaskProgress";
import { useAppStore } from "@/stores/appStore";
import { useI18n } from "@/stores/i18nStore";
import { ThemeProvider } from "@/components/ThemeProvider";
import { FileList } from "@/components/FileList";
import { TaskConfigPanel } from "@/components/TaskConfigPanel";
import { ProgressPanel } from "@/components/ProgressPanel";
import { LanguageSwitcher } from "@/components/LanguageSwitcher";
import { ThemeSwitcher } from "@/components/ThemeSwitcher";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Lock } from "lucide-react";

function AppContent() {
  useTaskProgress();
  const i18n = useI18n((s) => s.t);

  return (
    <div className="flex flex-col h-screen">
      <header className="border-b bg-card">
        <div className="container flex items-center justify-between py-4">
          <div className="flex items-center gap-2">
            <Lock className="h-6 w-6 text-primary" />
            <h1 className="text-xl font-bold">{i18n.app.title}</h1>
            <span className="text-xs text-muted-foreground ml-2">
              {i18n.app.subtitle}
            </span>
          </div>
          <div className="flex items-center gap-2">
            <LanguageSwitcher />
            <ThemeSwitcher />
          </div>
        </div>
      </header>

      <main className="flex-1 container py-6 overflow-auto">
        <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
          <div className="space-y-6">
            <Card>
              <CardHeader>
                <CardTitle>{i18n.file.title}</CardTitle>
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
                <CardTitle>{i18n.config.title}</CardTitle>
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
          {i18n.app.footer}
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
