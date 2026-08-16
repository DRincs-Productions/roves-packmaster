import { createFileRoute, useNavigate } from "@tanstack/react-router";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { InstallerCard } from "@/components/installer-card";
import { type Platform, PlatformToggle } from "@/components/platform-toggle";
import {
  Accordion,
  AccordionContent,
  AccordionItem,
  AccordionTrigger,
} from "@/components/ui/accordion";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { Textarea } from "@/components/ui/textarea";
import { readParentPackageJson } from "@/lib/release-info";
import { useSettings } from "@/lib/settings-context";
import { checkShellAvailability } from "@/lib/shell-availability";

export const Route = createFileRoute("/configure")({
  component: ConfigureView,
});

const PORTABLE_PLATFORMS: Platform[] = ["windows", "linux", "macos"];

// Only one real installer format exists per platform today — see README.md's own
// "nsis/rpm/appimage aren't implemented yet" — but each card's format picker is a
// multi-select regardless (see installer-card.tsx), so a second format later just means
// adding an entry here, not reshaping the UI.
const INSTALLER_FORMATS: Record<Platform, { value: string; label: string }[]> = {
  windows: [{ value: "msi", label: ".msi" }],
  linux: [{ value: "deb", label: ".deb" }],
  macos: [{ value: "dmg", label: ".dmg" }],
};

function ConfigureView() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const { settings, updateSettings } = useSettings();
  const [availability, setAvailability] = useState<Record<string, boolean> | null>(null);
  const [releaseInfo, setReleaseInfo] = useState({ name: "", version: "" });
  const [versionFromPackageJson, setVersionFromPackageJson] = useState(false);

  // A direct navigation here (or a reload) with no source picked yet has
  // nothing to configure a release for — send the user back to pick one.
  useEffect(() => {
    if (!settings.sourceDir) {
      navigate({ to: "/" });
    }
  }, [settings.sourceDir, navigate]);

  // Real, live check ("is this actually distributable") against the targeted shell
  // release's actual assets — not assumed just because it's a supported platform.
  useEffect(() => {
    checkShellAvailability([...PORTABLE_PLATFORMS]).then(setAvailability);
  }, []);

  // Derives name/version for this exact source folder. package.json's own version always
  // wins when present — a developer bumping it between builds shouldn't have to notice and
  // re-type it here — while the name, and the version when no package.json exists, fall
  // back to whatever was remembered for this same folder last time. Both stay editable.
  //
  // Only re-derive when the folder itself changes — settings.releaseInfoByPath and
  // updateSettings are read via closure on purpose (this effect is what writes the former);
  // listing them would re-run this on every keystroke below and clobber in-progress edits.
  // biome-ignore lint/correctness/useExhaustiveDependencies: see comment above
  useEffect(() => {
    const sourceDir = settings.sourceDir;
    if (!sourceDir) return;
    let cancelled = false;
    readParentPackageJson(sourceDir).then((pkg) => {
      if (cancelled) return;
      const remembered = settings.releaseInfoByPath[sourceDir];
      const next = {
        name: remembered?.name || pkg?.name || "",
        version: pkg?.version || remembered?.version || "",
      };
      setReleaseInfo(next);
      setVersionFromPackageJson(Boolean(pkg?.version));
      updateSettings({ releaseInfoByPath: { ...settings.releaseInfoByPath, [sourceDir]: next } });
    });
    return () => {
      cancelled = true;
    };
  }, [settings.sourceDir]);

  const updateReleaseInfo = (patch: Partial<{ name: string; version: string }>) => {
    if (!settings.sourceDir) return;
    const next = { ...releaseInfo, ...patch };
    setReleaseInfo(next);
    updateSettings({
      releaseInfoByPath: { ...settings.releaseInfoByPath, [settings.sourceDir]: next },
    });
  };

  if (!settings.sourceDir) return null;

  const unavailablePlatforms = PORTABLE_PLATFORMS.filter((p) => availability?.[p] === false);

  return (
    <div className="flex w-full max-w-2xl flex-col gap-6">
      <div>
        <h1 className="text-xl font-semibold">{t("configure.title")}</h1>
        <p className="text-muted-foreground text-sm">
          {t("configure.sourceLabel")}: <span className="font-mono">{settings.sourceDir}</span>{" "}
          <button
            type="button"
            className="underline underline-offset-2"
            onClick={() => navigate({ to: "/" })}
          >
            {t("configure.changeSource")}
          </button>
        </p>
      </div>

      <Card>
        <CardHeader>
          <CardTitle>{t("configure.releaseInfo.title")}</CardTitle>
          <CardDescription>{t("configure.releaseInfo.description")}</CardDescription>
        </CardHeader>
        <CardContent className="flex flex-col gap-4">
          <div className="flex flex-col gap-1.5">
            <Label htmlFor="game-name">{t("configure.releaseInfo.nameLabel")}</Label>
            <Input
              id="game-name"
              value={releaseInfo.name}
              placeholder={t("configure.releaseInfo.namePlaceholder")}
              onChange={(e) => updateReleaseInfo({ name: e.target.value })}
            />
          </div>
          <div className="flex flex-col gap-1.5">
            <Label htmlFor="game-version">{t("configure.releaseInfo.versionLabel")}</Label>
            <Input
              id="game-version"
              value={releaseInfo.version}
              placeholder="1.0.0"
              onChange={(e) => updateReleaseInfo({ version: e.target.value })}
            />
            {versionFromPackageJson && (
              <p className="text-muted-foreground text-xs">
                {t("configure.releaseInfo.versionFromPackageJson")}
              </p>
            )}
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>{t("configure.portable.title")}</CardTitle>
          <CardDescription>{t("configure.portable.description")}</CardDescription>
        </CardHeader>
        <CardContent className="flex flex-col gap-2">
          <div className="flex gap-3">
            {PORTABLE_PLATFORMS.map((p) => {
              const isAvailable = availability?.[p] ?? true;
              return (
                <PlatformToggle
                  key={p}
                  platform={p}
                  label={t(`configure.portable.${p}`)}
                  selected={isAvailable && settings.portable[p]}
                  disabled={!isAvailable}
                  onSelectedChange={(selected) =>
                    updateSettings({ portable: { ...settings.portable, [p]: selected } })
                  }
                />
              );
            })}
          </div>
          {unavailablePlatforms.length > 0 && (
            <p className="text-muted-foreground text-xs">
              {t("configure.portable.shellUnavailable", {
                platforms: unavailablePlatforms.map((p) => t(`system.${p}`)).join(", "),
              })}
            </p>
          )}
        </CardContent>
      </Card>

      <div className="flex flex-col gap-3">
        <div>
          <h2 className="text-lg font-semibold">{t("configure.installers.title")}</h2>
          <p className="text-muted-foreground text-sm">{t("configure.installers.description")}</p>
        </div>
        <div className="flex gap-3">
          {PORTABLE_PLATFORMS.map((p) => (
            <InstallerCard
              key={p}
              platform={p}
              title={t(`configure.installers.${p}`)}
              typeLabel={t("configure.installers.typeLabel")}
              enabled={settings.installers[p].enabled}
              onEnabledChange={(enabled) =>
                updateSettings({
                  installers: {
                    ...settings.installers,
                    [p]: { ...settings.installers[p], enabled },
                  },
                })
              }
              availableFormats={INSTALLER_FORMATS[p]}
              formats={settings.installers[p].formats}
              onFormatsChange={(formats) =>
                updateSettings({
                  installers: {
                    ...settings.installers,
                    [p]: { ...settings.installers[p], formats },
                  },
                })
              }
            />
          ))}
        </div>
      </div>

      <div className="flex flex-col gap-3">
        <h2 className="text-lg font-semibold">{t("configure.advanced.title")}</h2>
        <Accordion className="gap-4" defaultValue={["compression"]}>
          <AccordionItem
            value="steam"
            className="rounded-xl border bg-card px-4 shadow-xs ring-1 ring-foreground/10"
          >
            <AccordionTrigger>{t("configure.steam.title")}</AccordionTrigger>
            <AccordionContent className="flex flex-col gap-3">
              <p className="text-muted-foreground text-sm">{t("configure.steam.description")}</p>
              <p className="text-muted-foreground text-sm">{t("configure.steam.apiHint")}</p>
            </AccordionContent>
          </AccordionItem>

          <AccordionItem
            value="compression"
            className="rounded-xl border bg-card px-4 shadow-xs ring-1 ring-foreground/10"
          >
            <AccordionTrigger>{t("configure.compression.title")}</AccordionTrigger>
            <AccordionContent className="flex flex-col gap-4">
              <p className="text-muted-foreground text-sm">
                {t("configure.compression.description")}
              </p>
              <div className="flex items-center justify-between gap-4">
                <Label>{t("configure.compression.enable")}</Label>
                <Switch
                  checked={settings.compression.enabled}
                  onCheckedChange={(checked) =>
                    updateSettings({
                      compression: { ...settings.compression, enabled: checked },
                    })
                  }
                />
              </div>
              {settings.compression.enabled && (
                <>
                  <div className="flex flex-col gap-1.5">
                    <Label htmlFor="compression-level">
                      {t("configure.compression.levelLabel")}
                    </Label>
                    <p className="text-muted-foreground text-xs">
                      {t("configure.compression.levelDescription")}
                    </p>
                    <Input
                      id="compression-level"
                      type="number"
                      min={1}
                      max={19}
                      value={settings.compression.level}
                      onChange={(e) =>
                        updateSettings({
                          compression: {
                            ...settings.compression,
                            level: Number(e.target.value),
                          },
                        })
                      }
                    />
                  </div>
                  <div className="flex flex-col gap-1.5">
                    <Label htmlFor="max-pack-size">
                      {t("configure.compression.maxPackSizeLabel")}
                    </Label>
                    <p className="text-muted-foreground text-xs">
                      {t("configure.compression.maxPackSizeDescription")}
                    </p>
                    <Input
                      id="max-pack-size"
                      value={settings.compression.maxPackSize}
                      onChange={(e) =>
                        updateSettings({
                          compression: {
                            ...settings.compression,
                            maxPackSize: e.target.value,
                          },
                        })
                      }
                    />
                  </div>
                  <div className="flex flex-col gap-1.5">
                    <Label htmlFor="exclude">{t("configure.compression.excludeLabel")}</Label>
                    <p className="text-muted-foreground text-xs">
                      {t("configure.compression.excludeDescription")}
                    </p>
                    <Textarea
                      id="exclude"
                      placeholder={t("configure.compression.excludePlaceholder")}
                      value={settings.compression.exclude.join("\n")}
                      onChange={(e) =>
                        updateSettings({
                          compression: {
                            ...settings.compression,
                            exclude: e.target.value.split("\n").filter(Boolean),
                          },
                        })
                      }
                    />
                  </div>
                  <div className="flex flex-col gap-1.5">
                    <Label htmlFor="boot-include">
                      {t("configure.compression.bootIncludeLabel")}
                    </Label>
                    <p className="text-muted-foreground text-xs">
                      {t("configure.compression.bootIncludeDescription")}
                    </p>
                    <Textarea
                      id="boot-include"
                      placeholder={t("configure.compression.bootIncludePlaceholder")}
                      value={settings.compression.bootInclude.join("\n")}
                      onChange={(e) =>
                        updateSettings({
                          compression: {
                            ...settings.compression,
                            bootInclude: e.target.value.split("\n").filter(Boolean),
                          },
                        })
                      }
                    />
                  </div>
                </>
              )}
            </AccordionContent>
          </AccordionItem>
        </Accordion>
      </div>

      <Button
        type="button"
        size="lg"
        className="self-end"
        onClick={() => navigate({ to: "/generating" })}
      >
        {t("configure.startButton")}
      </Button>
    </div>
  );
}
