import { Sparkle } from "@phosphor-icons/react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import {
  checkForNewShellVersion,
  ROVES_RELEASES_URL,
  type ShellVersionCheckResult,
} from "@/lib/shell-version";

// Shown in the root layout (so it's visible regardless of which screen the user is on)
// whenever the Roves engine shell has a newer published release than the one this build
// of Packmaster currently targets (see src/lib/shell-version.ts).
export function ShellUpdateBanner() {
  const { t } = useTranslation();
  const [result, setResult] = useState<ShellVersionCheckResult | null>(null);

  useEffect(() => {
    let cancelled = false;
    checkForNewShellVersion().then((checked) => {
      if (!cancelled) setResult(checked);
    });
    return () => {
      cancelled = true;
    };
  }, []);

  if (!result?.isUpdateAvailable) return null;

  return (
    <Alert className="rounded-none border-x-0 border-t-0">
      <Sparkle weight="fill" />
      <AlertTitle>{t("shellUpdate.title")}</AlertTitle>
      <AlertDescription>
        <p>{t("shellUpdate.description", { current: result.current, latest: result.latest })}</p>
        <button
          type="button"
          className="cursor-pointer underline underline-offset-3 hover:text-foreground"
          onClick={() => openUrl(ROVES_RELEASES_URL)}
        >
          {t("shellUpdate.link")}
        </button>
      </AlertDescription>
    </Alert>
  );
}
