import { ArrowSquareOut, DownloadSimple, Info } from "@phosphor-icons/react";
import { getVersion } from "@tauri-apps/api/app";
import { openUrl } from "@tauri-apps/plugin-opener";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import roversIcon from "@/assets/roves-icon.svg";
import { Button } from "@/components/ui/button";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { Separator } from "@/components/ui/separator";
import {
  checkForNewShellVersion,
  ROVES_RELEASES_URL,
  ROVES_WEBSITE_URL,
  type ShellVersionCheckResult,
  TARGET_SHELL_VERSION,
} from "@/lib/shell-version";

/**
 * The single "what is this, what version am I on, is a newer shell out there"
 * entry point -- replaces the old always-visible ShellUpdateBanner. An update
 * is now surfaced as a small dot on this trigger itself (so it's still
 * noticeable without opening anything) rather than a persistent banner.
 */
export function InfoPopover() {
  const { t } = useTranslation();
  const [packmasterVersion, setPackmasterVersion] = useState<string | null>(null);
  const [shellCheck, setShellCheck] = useState<ShellVersionCheckResult | null>(null);

  useEffect(() => {
    getVersion().then(setPackmasterVersion);
    checkForNewShellVersion().then(setShellCheck);
  }, []);

  const updateAvailable = shellCheck?.isUpdateAvailable ?? false;

  return (
    <Popover>
      <PopoverTrigger
        render={
          <Button
            type="button"
            variant="ghost"
            size="icon-sm"
            aria-label={t("info.trigger")}
            className="relative"
          >
            <Info />
            {updateAvailable && (
              <span
                aria-hidden="true"
                className="bg-primary ring-background absolute right-0 bottom-0 size-2 rounded-full ring-2"
              />
            )}
          </Button>
        }
      />
      <PopoverContent align="start" className="w-80">
        <div className="flex items-center gap-2.5">
          <img src={roversIcon} alt="" className="size-8" />
          <span className="font-heading text-xl leading-none tracking-wide">Roves</span>
        </div>
        <p className="text-muted-foreground text-sm">{t("info.description")}</p>
        <Separator />
        <dl className="flex flex-col gap-1.5 text-sm">
          <div className="flex items-center justify-between gap-2">
            <dt className="text-muted-foreground">{t("info.packmasterVersion")}</dt>
            <dd className="font-mono">{packmasterVersion ? `v${packmasterVersion}` : "…"}</dd>
          </div>
          <div className="flex items-center justify-between gap-2">
            <dt className="text-muted-foreground">{t("info.shellVersion")}</dt>
            <dd className="font-mono">{TARGET_SHELL_VERSION}</dd>
          </div>
          <div className="flex items-center justify-between gap-2">
            <dt className="text-muted-foreground">{t("info.shellStatus")}</dt>
            <dd>{updateAvailable ? t("info.updateAvailable") : t("info.upToDate")}</dd>
          </div>
        </dl>
        {updateAvailable && (
          <Button type="button" size="sm" onClick={() => openUrl(ROVES_RELEASES_URL)}>
            <DownloadSimple />
            {t("info.downloadLatest")}
          </Button>
        )}
        <Separator />
        <button
          type="button"
          className="text-muted-foreground hover:text-foreground flex items-center gap-1.5 text-xs underline-offset-3 hover:underline"
          onClick={() => openUrl(ROVES_WEBSITE_URL)}
        >
          {ROVES_WEBSITE_URL.replace(/^https?:\/\//, "")}
          <ArrowSquareOut />
        </button>
      </PopoverContent>
    </Popover>
  );
}
