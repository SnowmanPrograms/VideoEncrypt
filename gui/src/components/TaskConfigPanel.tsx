import { useAppStore } from "@/stores/appStore";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { selectFiles, selectFolder, startTask } from "@/lib/tauri";
import { FolderOpen, FileUp, Lock, Unlock, Loader2 } from "lucide-react";
import type { TaskMode } from "@/types";

export function TaskConfigPanel() {
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
          Select Files
        </Button>
        <Button
          variant="outline"
          className="flex-1"
          onClick={handleSelectFolder}
          disabled={isProcessing}
        >
          <FolderOpen className="h-4 w-4 mr-2" />
          Select Folder
        </Button>
      </div>

      <Card>
        <CardHeader className="pb-3">
          <CardTitle className="text-base">Operation Mode</CardTitle>
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
              Encrypt
            </Button>
            <Button
              variant={taskMode === "Decrypt" ? "default" : "outline"}
              className="flex-1"
              onClick={() => setTaskMode("Decrypt")}
              disabled={isProcessing}
            >
              <Unlock className="h-4 w-4 mr-2" />
              Decrypt
            </Button>
          </div>
        </CardContent>
      </Card>

      <div className="space-y-4">
        <div className="space-y-2">
          <Label htmlFor="password">Password</Label>
          <Input
            id="password"
            type="password"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            placeholder="Enter password"
            disabled={isProcessing}
          />
        </div>

        {taskMode === "Encrypt" && (
          <div className="space-y-2">
            <Label htmlFor="confirm-password">Confirm Password</Label>
            <Input
              id="confirm-password"
              type="password"
              value={confirmPassword}
              onChange={(e) => setConfirmPassword(e.target.value)}
              placeholder="Confirm password"
              disabled={isProcessing}
              className={
                confirmPassword && password !== confirmPassword
                  ? "border-destructive"
                  : ""
              }
            />
            {confirmPassword && password !== confirmPassword && (
              <p className="text-xs text-destructive">Passwords do not match</p>
            )}
          </div>
        )}
      </div>

      {taskMode === "Encrypt" && (
        <Card>
          <CardHeader className="pb-3">
            <CardTitle className="text-base">Encryption Options</CardTitle>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="flex items-center justify-between">
              <div className="space-y-0.5">
                <Label htmlFor="encrypt-audio">Encrypt Audio</Label>
                <p className="text-xs text-muted-foreground">
                  Also encrypt audio tracks (slower)
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
                <Label htmlFor="scrub-metadata">Scrub Metadata</Label>
                <p className="text-xs text-muted-foreground">
                  Remove sensitive metadata (GPS, titles)
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
          <Label htmlFor="use-wal">Crash Safety (WAL)</Label>
          <p className="text-xs text-muted-foreground">
            Enable write-ahead logging for crash recovery
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
            Processing...
          </>
        ) : (
          <>
            {taskMode === "Encrypt" ? (
              <>
                <Lock className="h-4 w-4 mr-2" />
                Start Encryption
              </>
            ) : (
              <>
                <Unlock className="h-4 w-4 mr-2" />
                Start Decryption
              </>
            )}
          </>
        )}
      </Button>
    </div>
  );
}
