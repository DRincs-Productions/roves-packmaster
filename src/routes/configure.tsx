import { createFileRoute, useNavigate } from "@tanstack/react-router";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  Accordion,
  AccordionContent,
  AccordionItem,
  AccordionTrigger,
} from "@/components/ui/accordion";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
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
import { getHostPlatform, type HostPlatform } from "@/lib/platform";
import { useSettings } from "@/lib/settings-context";

export const Route = createFileRoute("/configure")({
  component: ConfigureView,
});

const INSTALLER_PLATFORMS = [
  {
    key: "windows" as const,
    requires: "windows" as HostPlatform,
    format: "msi" as const,
  },
  {
    key: "linux" as const,
    requires: "linux" as HostPlatform,
    format: "deb" as const,
  },
  {
    key: "macos" as const,
    requires: "macos" as HostPlatform,
    format: "dmg" as const,
  },
];

const PORTABLE_PLATFORMS = ["windows", "linux", "macos"] as const;

function ConfigureView() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const { settings, updateSettings } = useSettings();
  const [hostPlatform, setHostPlatform] = useState<HostPlatform | null>(null);

  useEffect(() => {
    getHostPlatform().then(setHostPlatform);
  }, []);

  // A direct navigation here (or a reload) with no source picked yet has
  // nothing to configure a release for — send the user back to pick one.
  useEffect(() => {
    if (!settings.sourceDir) {
      navigate({ to: "/" });
    }
  }, [settings.sourceDir, navigate]);

  if (!settings.sourceDir || !hostPlatform) return null;

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

      <Accordion defaultValue={["portable", "installers", "plugins", "compression"]}>
        <AccordionItem value="portable">
          <AccordionTrigger>{t("configure.portable.title")}</AccordionTrigger>
          <AccordionContent className="flex flex-col gap-4">
            <p className="text-muted-foreground text-sm">{t("configure.portable.description")}</p>
            <div className="flex flex-col gap-3">
              {PORTABLE_PLATFORMS.map((p) => (
                // biome-ignore lint/a11y/noLabelWithoutControl: Checkbox (Base UI) renders a real <button>, a labelable element nesting it inside <label> validly associates the two.
                <label key={p} className="flex items-center gap-2 text-sm">
                  <Checkbox
                    checked={settings.portable[p]}
                    onCheckedChange={(checked) =>
                      updateSettings({
                        portable: {
                          ...settings.portable,
                          [p]: checked === true,
                        },
                      })
                    }
                  />
                  {t(`configure.portable.${p}`)}
                </label>
              ))}
            </div>
          </AccordionContent>
        </AccordionItem>

        <AccordionItem value="installers">
          <AccordionTrigger>{t("configure.installers.title")}</AccordionTrigger>
          <AccordionContent className="flex flex-col gap-4">
            <p className="text-muted-foreground text-sm">{t("configure.installers.description")}</p>
            <div className="flex flex-col gap-4">
              {INSTALLER_PLATFORMS.map(({ key, requires, format }) => {
                const available = hostPlatform === requires;
                if (!available) {
                  return (
                    <div key={key} className="flex flex-col gap-1 text-sm opacity-60">
                      <span>{t(`configure.installers.${key}`)}</span>
                      <span className="text-muted-foreground text-xs">
                        {t("configure.installers.unavailableOnThisSystem", {
                          system: t(`system.${requires}`),
                        })}
                      </span>
                    </div>
                  );
                }
                const installer = settings.installers[key];
                return (
                  <div key={key} className="flex items-center justify-between gap-4">
                    {/* biome-ignore lint/a11y/noLabelWithoutControl: same Checkbox-in-label pattern as the portable section above. */}
                    <label className="flex items-center gap-2 text-sm">
                      <Checkbox
                        checked={installer.enabled}
                        onCheckedChange={(checked) =>
                          updateSettings({
                            installers: {
                              ...settings.installers,
                              [key]: {
                                ...installer,
                                enabled: checked === true,
                              },
                            },
                          })
                        }
                      />
                      {t(`configure.installers.${key}`)}
                    </label>
                    {installer.enabled && (
                      <Select value={format}>
                        <SelectTrigger size="sm" className="w-28">
                          <SelectValue />
                        </SelectTrigger>
                        <SelectContent>
                          {/* Only one format exists per platform today (see
                              the engine's own README, "nsis/rpm/appimage
                              aren't implemented yet") — this select exists
                              so adding one later doesn't need new UI. */}
                          <SelectItem value={format}>.{format}</SelectItem>
                        </SelectContent>
                      </Select>
                    )}
                  </div>
                );
              })}
            </div>
          </AccordionContent>
        </AccordionItem>

        <AccordionItem value="plugins">
          <AccordionTrigger>{t("configure.plugins.title")}</AccordionTrigger>
          <AccordionContent className="flex flex-col gap-4">
            <p className="text-muted-foreground text-sm">{t("configure.plugins.description")}</p>
            <div className="flex items-center justify-between gap-4">
              <div>
                <Label>{t("configure.plugins.steam")}</Label>
                <p className="text-muted-foreground text-sm">
                  {t("configure.plugins.steamDescription")}
                </p>
              </div>
              <Switch
                checked={settings.plugins.steam}
                onCheckedChange={(checked) => updateSettings({ plugins: { steam: checked } })}
              />
            </div>
          </AccordionContent>
        </AccordionItem>

        <AccordionItem value="compression">
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
                  <Label htmlFor="compression-level">{t("configure.compression.levelLabel")}</Label>
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
