import { FolderOpen, WarningCircle } from "@phosphor-icons/react";
import { createFileRoute, useNavigate } from "@tanstack/react-router";
import { open } from "@tauri-apps/plugin-dialog";
import { exists } from "@tauri-apps/plugin-fs";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Label } from "@/components/ui/label";
import { useSettings } from "@/lib/settings-context";

export const Route = createFileRoute("/")({
  component: SourceView,
});

function SourceView() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const { settings, updateSettings } = useSettings();
  const [selectedDir, setSelectedDir] = useState(settings.sourceDir);
  const [isAnalyzing, setIsAnalyzing] = useState(false);
  const [error, setError] = useState(false);

  const handleBrowse = async () => {
    const dir = await open({
      directory: true,
      multiple: false,
      title: t("source.browseDialogTitle"),
    });
    if (typeof dir === "string") {
      setSelectedDir(dir);
      setError(false);
    }
  };

  const handleContinue = async () => {
    if (!selectedDir) return;
    setIsAnalyzing(true);
    setError(false);
    try {
      // The one heuristic that doesn't need the real shell backend yet: a
      // built web app's output always has an index.html at its root (Vite,
      // and every other bundler, all produce one there).
      const separator = selectedDir.includes("\\") ? "\\" : "/";
      const hasIndexHtml = await exists(`${selectedDir}${separator}index.html`);
      if (!hasIndexHtml) {
        setError(true);
        return;
      }
      updateSettings({ sourceDir: selectedDir });
      navigate({ to: "/configure" });
    } finally {
      setIsAnalyzing(false);
    }
  };

  return (
    <Card className="w-full max-w-xl">
      <CardHeader>
        <CardTitle className="text-xl">{t("source.title")}</CardTitle>
        <CardDescription>
          {t("source.description", {
            dist: "dist/",
            command: "npm run build",
            packageJson: "package.json",
          })}
        </CardDescription>
      </CardHeader>
      <CardContent className="flex flex-col gap-4">
        <div className="flex flex-col gap-2">
          <Label htmlFor="source-dir">{t("source.pathLabel")}</Label>
          <div className="flex gap-2">
            <div
              id="source-dir"
              className="border-input bg-transparent text-muted-foreground flex h-9 flex-1 items-center truncate rounded-md border px-3 text-sm"
            >
              {selectedDir ?? t("source.pathPlaceholder")}
            </div>
            <Button type="button" variant="outline" onClick={handleBrowse}>
              <FolderOpen />
              {t("common.browse")}
            </Button>
          </div>
        </div>

        {error && (
          <Alert variant="destructive">
            <WarningCircle />
            <AlertTitle>{t("source.errorTitle")}</AlertTitle>
            <AlertDescription>
              {t("source.errorDescription", {
                indexHtml: "index.html",
                dist: "dist/",
              })}
            </AlertDescription>
          </Alert>
        )}

        <Button
          type="button"
          className="self-end"
          disabled={!selectedDir || isAnalyzing}
          onClick={handleContinue}
        >
          {isAnalyzing ? t("source.analyzing") : t("common.continue")}
        </Button>
      </CardContent>
    </Card>
  );
}
