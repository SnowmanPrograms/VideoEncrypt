import { useAppStore } from "@/stores/appStore";
import { useI18n } from "@/stores/i18nStore";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { selectFiles, selectFolder, startTask } from "@/lib/tauri";
import { FolderOpen, FileUp, Lock, Unlock, Loader2 } from "lucide-react";

export function TaskConfigPanel() {
  const i18n = useI18n((s) => s.t);
  const {
    files,
    addFiles,
    taskMode,
    setTaskMode,
    password,
    setPassword,
    confirmPassword,
    setConfirmPassword,
    encryptAudio,
    setEncryptAudio,
    scrubMetadata,
    setScrubMetadata,
    useWal,
    setUseWal,
    isProcessing,
    setIsProcessing,
    setCurrentTask,
  } = useAppStore();

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

  const handleStartTask = async () => {
    if (files.length === 0 || !password) return;
    if (taskMode === "Encrypt" && password !== confirmPassword) return;

    setIsProcessing(true);

    try {
      const taskId = await startTask({
        files: files.map((f) => f.path),
        password,
        mode: taskMode,
        encrypt_audio: encryptAudio,
        scrub_metadata: scrubMetadata,
        use_wal: useWal,
      });

      setCurrentTask({ id: taskId } as any);
    } catch (e) {
      console.error("Failed to start task:", e);
      setIsProcessing(false);
    }
  };

  const canStart =
    files.length > 0 &&
    password.length > 0 &&
    (taskMode === "Decrypt" || password === confirmPassword);

  return (
    <div className="space-y-6">
      <div className="flex gap-2">
        <Button
          variant="outline"
          className="flex-1"
          onClick={handleSelectFiles}
          disabled={isProcessing}
        >
          <FileUp className="h-4 w-4 mr-2" />
          {i18n.button.selectFiles}
        </Button>
        <Button
          variant="outline"
          className="flex-1"
          onClick={handleSelectFolder}
          disabled={isProcessing}
        >
          <FolderOpen className="h-4 w-4 mr-2" />
          {i18n.button.selectFolder}
        </Button>
      </div>

      <Card>
        <CardHeader className="pb-3">
          <CardTitle className="text-base">{i18n.config.mode}</CardTitle>
        </CardHeader>
        <CardContent>
          <div className="flex gap-2">
            <Button
              variant={taskMode === "Encrypt" ? "default" : "outline"}
              className="flex-1"
              onClick={() => setTaskMode("Encrypt")}
              disabled={isProcessing}
            >
              <Lock className="h-4 w-4 mr-2" />
              {i18n.config.encrypt}
            </Button>
            <Button
              variant={taskMode === "Decrypt" ? "default" : "outline"}
              className="flex-1"
              onClick={() => setTaskMode("Decrypt")}
              disabled={isProcessing}
            >
              <Unlock className="h-4 w-4 mr-2" />
              {i18n.config.decrypt}
            </Button>
          </div>
        </CardContent>
      </Card>

      <div className="space-y-4">
        <div className="space-y-2">
          <Label htmlFor="password">{i18n.config.password}</Label>
          <Input
            id="password"
            type="password"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            placeholder={i18n.config.passwordPlaceholder}
            disabled={isProcessing}
          />
        </div>

        {taskMode === "Encrypt" && (
          <div className="space-y-2">
            <Label htmlFor="confirm-password">{i18n.config.confirmPassword}</Label>
            <Input
              id="confirm-password"
              type="password"
              value={confirmPassword}
              onChange={(e) => setConfirmPassword(e.target.value)}
              placeholder={i18n.config.confirmPlaceholder}
              disabled={isProcessing}
              className={
                confirmPassword && password !== confirmPassword
                  ? "border-destructive"
                  : ""
              }
            />
            {confirmPassword && password !== confirmPassword && (
              <p className="text-xs text-destructive">{i18n.config.passwordMismatch}</p>
            )}
          </div>
        )}
      </div>

      {taskMode === "Encrypt" && (
        <Card>
          <CardHeader className="pb-3">
            <CardTitle className="text-base">{i18n.config.options}</CardTitle>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="flex items-center justify-between">
              <div className="space-y-0.5">
                <Label htmlFor="encrypt-audio">{i18n.config.encryptAudio}</Label>
                <p className="text-xs text-muted-foreground">
                  {i18n.config.encryptAudioHint}
                </p>
              </div>
              <Switch
                id="encrypt-audio"
                checked={encryptAudio}
                onCheckedChange={setEncryptAudio}
                disabled={isProcessing}
              />
            </div>

            <div className="flex items-center justify-between">
              <div className="space-y-0.5">
                <Label htmlFor="scrub-metadata">{i18n.config.scrubMetadata}</Label>
                <p className="text-xs text-muted-foreground">
                  {i18n.config.scrubMetadataHint}
                </p>
              </div>
              <Switch
                id="scrub-metadata"
                checked={scrubMetadata}
                onCheckedChange={setScrubMetadata}
                disabled={isProcessing}
              />
            </div>
          </CardContent>
        </Card>
      )}

      <div className="flex items-center justify-between">
        <div className="space-y-0.5">
          <Label htmlFor="use-wal">{i18n.config.crashSafety}</Label>
          <p className="text-xs text-muted-foreground">
            {i18n.config.crashSafetyHint}
          </p>
        </div>
        <Switch
          id="use-wal"
          checked={useWal}
          onCheckedChange={setUseWal}
          disabled={isProcessing}
        />
      </div>

      <Button
        className="w-full"
        size="lg"
        onClick={handleStartTask}
        disabled={!canStart || isProcessing}
      >
        {isProcessing ? (
          <>
            <Loader2 className="h-4 w-4 mr-2 animate-spin" />
            {i18n.config.processing}
          </>
        ) : taskMode === "Encrypt" ? (
          <>
            <Lock className="h-4 w-4 mr-2" />
            {i18n.config.startEncrypt}
          </>
        ) : (
          <>
            <Unlock className="h-4 w-4 mr-2" />
            {i18n.config.startDecrypt}
          </>
        )}
      </Button>
    </div>
  );
}
