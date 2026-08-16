import { CheckCircle, WarningCircle } from "@phosphor-icons/react";
import { createFileRoute, useNavigate } from "@tanstack/react-router";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { openPath } from "@tauri-apps/plugin-opener";
import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import roversIcon from "@/assets/roves-icon.svg";
import { Button } from "@/components/ui/button";
import { Progress } from "@/components/ui/progress";
import { useSettings } from "@/lib/settings-context";

export const Route = createFileRoute("/generating")({
  component: GeneratingView,
});

type Status = "running" | "done" | "error";

// Emitted from src-tauri/src/bundle.rs's `generate_release` — one selected platform can be
// mid-download while another is already packing, so this only ever drives an overall
// progress bar, not a strict linear percentage.
interface BundleProgressEvent {
  platform: string;
  phase: "checking" | "downloading" | "assembling" | "packing" | "zipping" | "done";
  fraction: number;
}

function GeneratingView() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const { settings } = useSettings();
  const [progress, setProgress] = useState(0);
  const [status, setStatus] = useState<Status>("running");
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const releaseDirRef = useRef<string | null>(null);

  useEffect(() => {
    if (!settings.sourceDir) {
      navigate({ to: "/" });
      return;
    }

    let cancelled = false;
    let unlisten: UnlistenFn | undefined;
    const platformProgress = new Map<string, number>();
    const platformCount = [
      settings.portable.windows,
      settings.portable.linux,
      settings.portable.macos,
    ].filter(Boolean).length;

    async function run() {
      unlisten = await listen<BundleProgressEvent>("bundle-progress", (event) => {
        platformProgress.set(event.payload.platform, event.payload.fraction);
        if (platformCount > 0) {
          const total = [...platformProgress.values()].reduce((sum, value) => sum + value, 0);
          setProgress(Math.round((total / platformCount) * 100));
        }
      });

      try {
        const releaseDir = await invoke<string>("generate_release", { settings });
        if (cancelled) return;
        releaseDirRef.current = releaseDir;
        setProgress(100);
        setStatus("done");
      } catch (error) {
        if (!cancelled) {
          setErrorMessage(error instanceof Error ? error.message : String(error));
          setStatus("error");
        }
      }
    }

    run();
    return () => {
      cancelled = true;
      unlisten?.();
    };
    // `navigate` (TanStack Router) is a stable reference across renders —
    // listing it doesn't cause this effect to re-run on every render, it's
    // just here to satisfy the lint rule honestly rather than suppress it.
  }, [settings, navigate]);

  const handleOpenFolder = async () => {
    if (releaseDirRef.current) {
      await openPath(releaseDirRef.current);
    }
  };

  return (
    <div className="flex flex-col items-center gap-8 text-center">
      <div className="flex items-center gap-4">
        <img src={roversIcon} alt="" className="h-16 w-16" />
        <span className="font-heading text-5xl leading-none tracking-wide">Roves</span>
      </div>

      {status === "running" && (
        <div className="flex w-72 flex-col gap-3">
          <Progress value={progress} />
          <p className="text-muted-foreground text-sm">{t("generating.title")}</p>
        </div>
      )}

      {status === "done" && (
        <div className="flex flex-col items-center gap-3">
          <CheckCircle className="text-primary size-10" weight="fill" />
          <h2 className="text-lg font-semibold">{t("generating.doneTitle")}</h2>
          <p className="text-muted-foreground max-w-sm text-sm">
            {t("generating.doneDescription", { folder: "release" })}
          </p>
          <div className="mt-2 flex gap-3">
            <Button type="button" variant="outline" onClick={() => navigate({ to: "/configure" })}>
              {t("generating.startOver")}
            </Button>
            <Button type="button" onClick={handleOpenFolder}>
              {t("generating.openFolder")}
            </Button>
          </div>
        </div>
      )}

      {status === "error" && (
        <div className="flex flex-col items-center gap-3">
          <WarningCircle className="text-destructive size-10" weight="fill" />
          <h2 className="text-lg font-semibold">{t("generating.errorTitle")}</h2>
          <p className="text-muted-foreground max-w-sm text-sm">
            {errorMessage ?? t("generating.errorGeneric")}
          </p>
          <Button type="button" variant="outline" onClick={() => navigate({ to: "/configure" })}>
            {t("generating.startOver")}
          </Button>
        </div>
      )}
    </div>
  );
}
