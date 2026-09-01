import { Image, WarningCircle } from "@phosphor-icons/react";
import { createFileRoute, useNavigate } from "@tanstack/react-router";
import { open } from "@tauri-apps/plugin-dialog";
import { exists } from "@tauri-apps/plugin-fs";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { InstallerCard } from "@/components/installer-card";
import { type MobilePlatform, MobilePlatformToggle } from "@/components/mobile-platform-toggle";
import { type Platform, PlatformToggle } from "@/components/platform-toggle";
import {
  Accordion,
  AccordionContent,
  AccordionItem,
  AccordionTrigger,
} from "@/components/ui/accordion";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import { Textarea } from "@/components/ui/textarea";
import { checkAndroidAvailability } from "@/lib/android-availability";
import {
  checkInstallerAvailability,
  type InstallerAvailability,
} from "@/lib/installer-availability";
import { readParentPackageJson } from "@/lib/release-info";
import type { MobileOrientation } from "@/lib/settings";
import { useSettings } from "@/lib/settings-context";
import { checkShellAvailability } from "@/lib/shell-availability";
import { readWebManifest, type WebManifestInfo } from "@/lib/web-manifest";

export const Route = createFileRoute("/configure")({
  component: ConfigureView,
});

const PORTABLE_PLATFORMS: Platform[] = ["windows", "linux", "macos"];

// Only "android" exists as a card today -- "ios" joins this list once that platform actually
// exists (see mobile-platform-toggle.tsx and settings.ts's own MobileSettings comment).
const MOBILE_PLATFORMS: MobilePlatform[] = ["android"];

const MOBILE_ORIENTATIONS: MobileOrientation[] = [
  "any",
  "natural",
  "landscape",
  "landscape-primary",
  "landscape-secondary",
  "portrait",
  "portrait-primary",
  "portrait-secondary",
];

// Only one real installer format exists per platform today — see README.md's own
// "nsis/rpm/appimage aren't implemented yet" — but each card's format picker is a
// multi-select regardless (see installer-card.tsx), so a second format later just means
// adding an entry here, not reshaping the UI.
const INSTALLER_FORMATS: Record<Platform, { value: string; label: string }[]> = {
  windows: [{ value: "msi", label: ".msi" }],
  linux: [{ value: "deb", label: ".deb" }],
  macos: [{ value: "dmg", label: ".dmg" }],
};

// Mirrors bundle.rs's own generate_release check (non-empty, digits only) -- kept in sync
// with that Rust-side validation rather than introduced independently.
const isValidSteamAppId = (appId: string) => /^[0-9]+$/.test(appId.trim());

function ConfigureView() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const { settings, updateSettings } = useSettings();
  const [availability, setAvailability] = useState<Record<string, boolean> | null>(null);
  const [installerAvailability, setInstallerAvailability] = useState<Record<
    string,
    InstallerAvailability
  > | null>(null);
  const [releaseInfo, setReleaseInfo] = useState({ name: "", version: "" });
  const [nameFromPackageJson, setNameFromPackageJson] = useState(false);
  const [versionFromPackageJson, setVersionFromPackageJson] = useState(false);
  const [openAccordionItems, setOpenAccordionItems] = useState<string[]>(["steam"]);
  const [showSteamAppIdError, setShowSteamAppIdError] = useState(false);
  const [webManifestInfo, setWebManifestInfo] = useState<WebManifestInfo | null>(null);
  const [useWebManifest, setUseWebManifest] = useState(true);
  const [androidAvailable, setAndroidAvailable] = useState<{
    available: boolean;
    reason: string | null;
  } | null>(null);
  const [detectedIconPath, setDetectedIconPath] = useState<string | null>(null);

  // A direct navigation here (or a reload) with no source picked yet has
  // nothing to configure a release for — send the user back to pick one.
  useEffect(() => {
    if (!settings.sourceDir) {
      navigate({ to: "/" });
    }
  }, [settings.sourceDir, navigate]);

  // Real, live check ("is this actually distributable") that this exact machine can build
  // an installer for each platform — right host OS *and* its native tool actually
  // installed (WiX/hdiutil/dpkg-deb). Unlike the shell download, there's no way around
  // this: an installer can only ever be built on its own platform.
  useEffect(() => {
    checkInstallerAvailability([...PORTABLE_PLATFORMS]).then(setInstallerAvailability);
  }, []);

  // Real, live check ("is this actually distributable") -- unlike desktop's portable
  // bundling, Android packaging currently only runs on Linux/macOS (see
  // src-tauri/src/android.rs's own doc comment for why).
  useEffect(() => {
    checkAndroidAvailability().then(setAndroidAvailable);
  }, []);

  // Real, live check ("is this actually distributable") against the targeted shell
  // release's actual assets — not assumed just because it's a supported platform. Re-run
  // whenever the Steam toggle changes: a platform whose plain shell is published might not
  // (yet) have a published Steam-enabled variant, and vice versa.
  useEffect(() => {
    checkShellAvailability([...PORTABLE_PLATFORMS], settings.plugins.steam.enabled).then(
      setAvailability,
    );
  }, [settings.plugins.steam.enabled]);

  // Derives name/version for this exact source folder. package.json's own value always wins
  // when present, for both fields — a developer bumping it (or renaming their package)
  // between builds shouldn't have to notice and re-type it here, and the field becomes
  // read-only below in that case so there's no illusion it can be edited from here at all.
  // With no package.json value, both fall back to whatever was remembered for this same
  // folder last time, and stay editable.
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
        name: pkg?.name || remembered?.name || "",
        version: pkg?.version || remembered?.version || "",
      };
      setReleaseInfo(next);
      setNameFromPackageJson(Boolean(pkg?.name));
      setVersionFromPackageJson(Boolean(pkg?.version));
      updateSettings({ releaseInfoByPath: { ...settings.releaseInfoByPath, [sourceDir]: next } });
    });
    return () => {
      cancelled = true;
    };
  }, [settings.sourceDir]);

  // Whether a project has its own web app manifest is a per-project fact, not a persisted
  // global preference (see settings.ts's MobileSettings comment) -- re-derived fresh every
  // time the source folder changes, defaulting the switch to "on" whenever one is found,
  // exactly as requested. The user can still flip it off within this session (e.g. to type
  // manual overrides even though a manifest exists), it just isn't remembered afterward.
  useEffect(() => {
    const sourceDir = settings.sourceDir;
    if (!sourceDir) return;
    let cancelled = false;
    readWebManifest(sourceDir).then((manifest) => {
      if (cancelled) return;
      setWebManifestInfo(manifest);
      setUseWebManifest(Boolean(manifest));
    });
    return () => {
      cancelled = true;
    };
  }, [settings.sourceDir]);

  // Same auto-detect default as bundle.rs's own `resolve_icon_path` -- an `icon.png` sitting
  // directly in the build output, shown here (not silently applied) so it's obvious an icon
  // is already going to be used even before an explicit one is ever picked.
  useEffect(() => {
    const sourceDir = settings.sourceDir;
    if (!sourceDir) return;
    let cancelled = false;
    const separator = sourceDir.includes("\\") ? "\\" : "/";
    const candidate = `${sourceDir}${sourceDir.endsWith(separator) ? "" : separator}icon.png`;
    exists(candidate).then((found) => {
      if (!cancelled) setDetectedIconPath(found ? candidate : null);
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
  const steamAppIdInvalid =
    settings.plugins.steam.enabled && !isValidSteamAppId(settings.plugins.steam.appId);

  // "Mobile" advanced settings only make sense once at least one mobile platform is enabled
  // (currently just Android) -- see settings.ts's MobileSettings comment on why these are
  // shared across mobile platforms rather than duplicated per platform.
  const anyMobileEnabled = (androidAvailable?.available ?? true) && settings.mobile.android.enabled;
  const manifestDriven = Boolean(webManifestInfo) && useWebManifest;
  const displayedAppName = manifestDriven
    ? webManifestInfo?.shortName || webManifestInfo?.name || ""
    : settings.mobile.advanced.appName;
  const displayedOrientation = manifestDriven
    ? ((webManifestInfo?.orientation as MobileOrientation | undefined) ?? "")
    : settings.mobile.advanced.orientation;

  const handleBrowseIcon = async () => {
    const path = await open({
      directory: false,
      multiple: false,
      title: t("configure.icon.browseTitle"),
      filters: [{ name: "PNG", extensions: ["png"] }],
    });
    if (typeof path === "string") {
      updateSettings({ icon: { path } });
    }
  };

  const handleGenerateClick = () => {
    if (steamAppIdInvalid) {
      setShowSteamAppIdError(true);
      setOpenAccordionItems((current) =>
        current.includes("steam") ? current : [...current, "steam"],
      );
      return;
    }
    navigate({ to: "/generating" });
  };

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
              disabled={nameFromPackageJson}
              onChange={(e) => updateReleaseInfo({ name: e.target.value })}
            />
            {nameFromPackageJson && (
              <p className="text-muted-foreground text-xs">
                {t("configure.releaseInfo.nameFromPackageJson")}
              </p>
            )}
          </div>
          <div className="flex flex-col gap-1.5">
            <Label htmlFor="game-version">{t("configure.releaseInfo.versionLabel")}</Label>
            <Input
              id="game-version"
              value={releaseInfo.version}
              placeholder="1.0.0"
              disabled={versionFromPackageJson}
              onChange={(e) => updateReleaseInfo({ version: e.target.value })}
            />
            {versionFromPackageJson && (
              <p className="text-muted-foreground text-xs">
                {t("configure.releaseInfo.versionFromPackageJson")}
              </p>
            )}
          </div>
          <div className="flex flex-col gap-1.5">
            <Label>{t("configure.releaseInfo.iconLabel")}</Label>
            <p className="text-muted-foreground text-xs">
              {t("configure.releaseInfo.iconDescription")}
            </p>
            <div className="flex items-center gap-2">
              <Button type="button" variant="outline" onClick={handleBrowseIcon}>
                <Image />
                {t("configure.icon.browse")}
              </Button>
              {(settings.icon.path || detectedIconPath) && (
                <span className="truncate text-muted-foreground text-xs">
                  {settings.icon.path ?? detectedIconPath}
                </span>
              )}
              {settings.icon.path && (
                <Button
                  type="button"
                  variant="ghost"
                  size="sm"
                  onClick={() => updateSettings({ icon: { path: null } })}
                >
                  {t("configure.icon.clear")}
                </Button>
              )}
            </div>
            {!settings.icon.path && detectedIconPath && (
              <p className="text-muted-foreground text-xs">
                {t("configure.releaseInfo.iconAutoDetected")}
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
            <p className="text-destructive flex items-center gap-1.5 text-xs">
              <WarningCircle className="size-3.5 shrink-0" weight="fill" />
              {t("configure.portable.shellUnavailable", {
                platforms: unavailablePlatforms.map((p) => t(`system.${p}`)).join(", "),
              })}
            </p>
          )}
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>{t("configure.installers.title")}</CardTitle>
          <CardDescription>{t("configure.installers.description")}</CardDescription>
        </CardHeader>
        <CardContent className="flex flex-col gap-2">
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
                      [p]: {
                        ...settings.installers[p],
                        enabled,
                        // Default to the one real format that exists per platform today, so
                        // enabling a card doesn't also require opening its (currently
                        // single-option) format picker just to get anything selected.
                        formats:
                          enabled && settings.installers[p].formats.length === 0
                            ? INSTALLER_FORMATS[p].map((f) => f.value)
                            : settings.installers[p].formats,
                      },
                    },
                  })
                }
                available={installerAvailability ? installerAvailability[p].available : null}
                unavailableReason={installerAvailability ? installerAvailability[p].reason : null}
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
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>{t("configure.mobile.title")}</CardTitle>
          <CardDescription>{t("configure.mobile.description")}</CardDescription>
        </CardHeader>
        <CardContent className="flex flex-col gap-4">
          <div className="flex gap-3">
            {MOBILE_PLATFORMS.map((p) => (
              <MobilePlatformToggle
                key={p}
                platform={p}
                label={t(`configure.mobile.${p}`)}
                selected={(androidAvailable?.available ?? true) && settings.mobile[p].enabled}
                disabled={androidAvailable !== null && !androidAvailable.available}
                onSelectedChange={(enabled) =>
                  updateSettings({ mobile: { ...settings.mobile, [p]: { enabled } } })
                }
              />
            ))}
          </div>
          {androidAvailable && !androidAvailable.available && (
            <p className="text-destructive flex items-center gap-1.5 text-xs">
              <WarningCircle className="size-3.5 shrink-0" weight="fill" />
              {androidAvailable.reason}
            </p>
          )}

          {anyMobileEnabled && (
            <Accordion className="gap-4">
              <AccordionItem
                value="mobile-advanced"
                className="rounded-xl border bg-card px-4 shadow-xs ring-1 ring-foreground/10"
              >
                <AccordionTrigger>{t("configure.mobileAdvanced.title")}</AccordionTrigger>
                <AccordionContent className="flex flex-col gap-4">
                  <p className="text-muted-foreground text-sm">
                    {t("configure.mobileAdvanced.description")}
                  </p>

                  {webManifestInfo ? (
                    <div className="flex items-center justify-between gap-4">
                      <div>
                        <Label>{t("configure.mobileAdvanced.useWebManifestLabel")}</Label>
                        <p className="text-muted-foreground text-xs">
                          {t("configure.mobileAdvanced.useWebManifestFound", {
                            file: webManifestInfo.file,
                          })}
                        </p>
                      </div>
                      <Switch checked={useWebManifest} onCheckedChange={setUseWebManifest} />
                    </div>
                  ) : (
                    <p className="text-muted-foreground text-xs">
                      {t("configure.mobileAdvanced.useWebManifestNotFound")}
                    </p>
                  )}

                  <div className="flex flex-col gap-1.5">
                    <Label htmlFor="mobile-app-name">
                      {t("configure.mobileAdvanced.appNameLabel")}
                    </Label>
                    <Input
                      id="mobile-app-name"
                      disabled={manifestDriven}
                      placeholder={t("configure.mobileAdvanced.appNamePlaceholder")}
                      value={displayedAppName}
                      onChange={(e) =>
                        updateSettings({
                          mobile: {
                            ...settings.mobile,
                            advanced: { ...settings.mobile.advanced, appName: e.target.value },
                          },
                        })
                      }
                    />
                  </div>

                  <div className="flex flex-col gap-1.5">
                    <Label htmlFor="mobile-orientation">
                      {t("configure.mobileAdvanced.orientationLabel")}
                    </Label>
                    <Select
                      disabled={manifestDriven}
                      value={displayedOrientation}
                      onValueChange={(value) =>
                        updateSettings({
                          mobile: {
                            ...settings.mobile,
                            advanced: {
                              ...settings.mobile.advanced,
                              orientation: (value as MobileOrientation) ?? "",
                            },
                          },
                        })
                      }
                    >
                      <SelectTrigger id="mobile-orientation" className="w-full">
                        <SelectValue />
                      </SelectTrigger>
                      <SelectContent>
                        {MOBILE_ORIENTATIONS.map((value) => (
                          <SelectItem key={value} value={value}>
                            {t(
                              `configure.mobileAdvanced.orientationOptions.${value.replace(/-/g, "_")}`,
                            )}
                          </SelectItem>
                        ))}
                      </SelectContent>
                    </Select>
                  </div>
                </AccordionContent>
              </AccordionItem>
            </Accordion>
          )}
        </CardContent>
      </Card>

      <div className="flex flex-col gap-3">
        <h2 className="text-lg font-semibold">{t("configure.advanced.title")}</h2>
        <Accordion
          className="gap-4"
          value={openAccordionItems}
          onValueChange={setOpenAccordionItems}
        >
          <AccordionItem
            value="steam"
            className="rounded-xl border bg-card px-4 shadow-xs ring-1 ring-foreground/10"
          >
            <AccordionTrigger>{t("configure.steam.title")}</AccordionTrigger>
            <AccordionContent className="flex flex-col gap-3">
              <p className="text-muted-foreground text-sm">{t("configure.steam.description")}</p>
              <div className="flex items-center justify-between gap-4">
                <Label>{t("configure.steam.enableLabel")}</Label>
                <Switch
                  checked={settings.plugins.steam.enabled}
                  onCheckedChange={(checked) =>
                    updateSettings({
                      plugins: { steam: { ...settings.plugins.steam, enabled: checked } },
                    })
                  }
                />
              </div>
              {settings.plugins.steam.enabled && (
                <div className="flex flex-col gap-1.5">
                  <Label htmlFor="steam-app-id">{t("configure.steam.appIdLabel")}</Label>
                  <p className="text-muted-foreground text-xs">
                    {t("configure.steam.appIdDescription")}
                  </p>
                  <Input
                    id="steam-app-id"
                    inputMode="numeric"
                    aria-invalid={showSteamAppIdError && steamAppIdInvalid}
                    placeholder={t("configure.steam.appIdPlaceholder")}
                    value={settings.plugins.steam.appId}
                    onChange={(e) => {
                      const digitsOnly = e.target.value.replace(/\D/g, "");
                      updateSettings({
                        plugins: { steam: { ...settings.plugins.steam, appId: digitsOnly } },
                      });
                    }}
                  />
                  {showSteamAppIdError && steamAppIdInvalid && (
                    <p className="text-destructive text-xs">{t("configure.steam.appIdError")}</p>
                  )}
                </div>
              )}
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

      {showSteamAppIdError && steamAppIdInvalid && (
        <Alert variant="destructive">
          <WarningCircle />
          <AlertTitle>{t("configure.errors.title")}</AlertTitle>
          <AlertDescription>{t("configure.steam.appIdError")}</AlertDescription>
        </Alert>
      )}

      <Button type="button" size="lg" className="self-end" onClick={handleGenerateClick}>
        {t("configure.startButton")}
      </Button>
    </div>
  );
}
