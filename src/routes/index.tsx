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
  const [errorReason, setErrorReason] = useState<"missing-index" | "looks-like-source" | null>(
    null,
  );

  const handleBrowse = async () => {
    const dir = await open({
      directory: true,
      multiple: false,
      title: t("source.browseDialogTitle"),
      // Start from wherever was picked last, if anything — same folder a game developer
      // rebuilds into most of the time.
      defaultPath: selectedDir ?? undefined,
    });
    if (typeof dir === "string") {
      setSelectedDir(dir);
      setErrorReason(null);
    }
  };

  const handleContinue = async () => {
    if (!selectedDir) return;
    setIsAnalyzing(true);
    setErrorReason(null);
    try {
      // The one heuristic that doesn't need the real shell backend yet: a
      // built web app's output always has an index.html at its root (Vite,
      // and every other bundler, all produce one there).
      const separator = selectedDir.includes("\\") ? "\\" : "/";
      const hasIndexHtml = await exists(`${selectedDir}${separator}index.html`);
      if (!hasIndexHtml) {
        setErrorReason("missing-index");
        return;
      }
      // A real built dist/ output never ships its own package.json alongside the game
      // files — only a project's *source* root does. Vite (and most bundlers) also emit
      // index.html at the source root itself (the template `npm run build` starts from),
      // so the index.html check alone can't tell "source root" and "built output" apart on
      // its own — this catches exactly that case (confirmed by a real report: picking a
      // Vite project's source root instead of its dist/ passed the index.html check clean).
      const hasPackageJson = await exists(`${selectedDir}${separator}package.json`);
      if (hasPackageJson) {
        setErrorReason("looks-like-source");
        return;
      }
      updateSettings({ sourceDir: selectedDir });
      navigate({ to: "/configure" });
    } finally {
      setIsAnalyzing(false);
    }
  };

  return (
    <Card className="my-auto w-full max-w-xl">
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

        {errorReason && (
          <Alert variant="destructive">
            <WarningCircle />
            <AlertTitle>{t("source.errorTitle")}</AlertTitle>
            <AlertDescription>
              {errorReason === "missing-index"
                ? t("source.errorDescription", {
                    indexHtml: "index.html",
                    dist: "dist/",
                  })
                : t("source.errorLooksLikeSource", {
                    packageJson: "package.json",
                    dist: "dist/",
                    command: "npm run build",
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
