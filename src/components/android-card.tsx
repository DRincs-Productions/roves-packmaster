import { Info } from "@phosphor-icons/react";
import { useTranslation } from "react-i18next";
import {
  Accordion,
  AccordionContent,
  AccordionItem,
  AccordionTrigger,
} from "@/components/ui/accordion";
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
import type { AndroidOrientation } from "@/lib/settings";

const ORIENTATIONS: AndroidOrientation[] = [
  "any",
  "natural",
  "landscape",
  "landscape-primary",
  "landscape-secondary",
  "portrait",
  "portrait-primary",
  "portrait-secondary",
];

interface AndroidCardProps {
  enabled: boolean;
  onEnabledChange: (enabled: boolean) => void;
  /** Filename actually found in the source folder (see readWebManifest), or null if none of
   * manifest.webmanifest/manifest.json/site.webmanifest exist there. */
  webManifestFile: string | null;
  /** Not part of PackmasterSettings -- see settings.ts's own AndroidSettings comment for why
   * this is per-project derived state, not a persisted global preference. */
  useWebManifest: boolean;
  onUseWebManifestChange: (value: boolean) => void;
  appName: string;
  onAppNameChange: (value: string) => void;
  orientation: AndroidOrientation | "";
  onOrientationChange: (value: AndroidOrientation | "") => void;
  themeColor: string;
  onThemeColorChange: (value: string) => void;
}

export function AndroidCard({
  enabled,
  onEnabledChange,
  webManifestFile,
  useWebManifest,
  onUseWebManifestChange,
  appName,
  onAppNameChange,
  orientation,
  onOrientationChange,
  themeColor,
  onThemeColorChange,
}: AndroidCardProps) {
  const { t } = useTranslation();
  const manifestDriven = Boolean(webManifestFile) && useWebManifest;

  return (
    <Card className="flex-1">
      <CardHeader className="flex-row items-center justify-between gap-2">
        <div>
          <CardTitle>{t("configure.mobile.android.title")}</CardTitle>
          <CardDescription>{t("configure.mobile.android.description")}</CardDescription>
        </div>
        <Switch checked={enabled} onCheckedChange={onEnabledChange} />
      </CardHeader>
      {enabled && (
        <CardContent className="flex flex-col gap-4">
          <p className="flex items-start gap-1.5 text-muted-foreground text-xs">
            <Info className="mt-0.5 size-3.5 shrink-0" />
            {t("configure.mobile.android.comingSoonNotice")}
          </p>

          <Accordion className="gap-4">
            <AccordionItem
              value="android-advanced"
              className="rounded-xl border bg-card px-4 shadow-xs ring-1 ring-foreground/10"
            >
              <AccordionTrigger>{t("configure.mobile.android.advancedTitle")}</AccordionTrigger>
              <AccordionContent className="flex flex-col gap-4">
                {webManifestFile ? (
                  <div className="flex items-center justify-between gap-4">
                    <div>
                      <Label>{t("configure.mobile.android.useWebManifestLabel")}</Label>
                      <p className="text-muted-foreground text-xs">
                        {t("configure.mobile.android.useWebManifestFound", {
                          file: webManifestFile,
                        })}
                      </p>
                    </div>
                    <Switch checked={useWebManifest} onCheckedChange={onUseWebManifestChange} />
                  </div>
                ) : (
                  <p className="text-muted-foreground text-xs">
                    {t("configure.mobile.android.useWebManifestNotFound")}
                  </p>
                )}

                <div className="flex flex-col gap-1.5">
                  <Label htmlFor="android-app-name">
                    {t("configure.mobile.android.appNameLabel")}
                  </Label>
                  <Input
                    id="android-app-name"
                    disabled={manifestDriven}
                    placeholder={t("configure.mobile.android.appNamePlaceholder")}
                    value={appName}
                    onChange={(e) => onAppNameChange(e.target.value)}
                  />
                </div>

                <div className="flex flex-col gap-1.5">
                  <Label htmlFor="android-orientation">
                    {t("configure.mobile.android.orientationLabel")}
                  </Label>
                  <Select
                    disabled={manifestDriven}
                    value={orientation}
                    onValueChange={(value) =>
                      onOrientationChange((value as AndroidOrientation) ?? "")
                    }
                  >
                    <SelectTrigger id="android-orientation" className="w-full">
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      {ORIENTATIONS.map((value) => (
                        <SelectItem key={value} value={value}>
                          {t(
                            `configure.mobile.android.orientationOptions.${value.replace(/-/g, "_")}`,
                          )}
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                </div>

                <div className="flex flex-col gap-1.5">
                  <Label htmlFor="android-theme-color">
                    {t("configure.mobile.android.themeColorLabel")}
                  </Label>
                  <Input
                    id="android-theme-color"
                    disabled={manifestDriven}
                    placeholder={t("configure.mobile.android.themeColorPlaceholder")}
                    value={themeColor}
                    onChange={(e) => onThemeColorChange(e.target.value)}
                  />
                </div>
              </AccordionContent>
            </AccordionItem>
          </Accordion>
        </CardContent>
      )}
    </Card>
  );
}
