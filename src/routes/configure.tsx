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
import { Switch } from "@/components/ui/switch";
import { Textarea } from "@/components/ui/textarea";
import { useSettings } from "@/lib/settings-context";
import { checkShellAvailability } from "@/lib/shell-availability";

export const Route = createFileRoute("/configure")({
  component: ConfigureView,
});

const PORTABLE_PLATFORMS = ["windows", "linux", "macos"] as const;

function ConfigureView() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const { settings, updateSettings } = useSettings();
  const [availability, setAvailability] = useState<Record<string, boolean> | null>(null);

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

  if (!settings.sourceDir) return null;

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

      <Accordion defaultValue={["portable", "compression"]}>
        <AccordionItem value="portable">
          <AccordionTrigger>{t("configure.portable.title")}</AccordionTrigger>
          <AccordionContent className="flex flex-col gap-4">
            <p className="text-muted-foreground text-sm">{t("configure.portable.description")}</p>
            <div className="flex flex-col gap-3">
              {PORTABLE_PLATFORMS.map((p) => {
                const isAvailable = availability?.[p] ?? true;
                return (
                  <div key={p} className="flex flex-col gap-1">
                    {/* biome-ignore lint/a11y/noLabelWithoutControl: Checkbox (Base UI) renders a real <button>, a labelable element nesting it inside <label> validly associates the two. */}
                    <label
                      className="flex items-center gap-2 text-sm data-[disabled]:opacity-60"
                      data-disabled={!isAvailable || undefined}
                    >
                      <Checkbox
                        checked={isAvailable && settings.portable[p]}
                        disabled={!isAvailable}
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
                    {!isAvailable && (
                      <span className="text-muted-foreground pl-6 text-xs">
                        {t("configure.portable.shellUnavailable")}
                      </span>
                    )}
                  </div>
                );
              })}
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
