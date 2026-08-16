import { PLATFORM_ICONS, type Platform } from "@/components/platform-toggle";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Checkbox } from "@/components/ui/checkbox";
import {
  Combobox,
  ComboboxChip,
  ComboboxChips,
  ComboboxChipsInput,
  ComboboxContent,
  ComboboxEmpty,
  ComboboxItem,
  ComboboxList,
  useComboboxAnchor,
} from "@/components/ui/combobox";
import { Label } from "@/components/ui/label";

interface FormatOption {
  value: string;
  label: string;
}

interface InstallerCardProps {
  platform: Platform;
  title: string;
  typeLabel: string;
  enabled: boolean;
  onEnabledChange: (enabled: boolean) => void;
  /** Only one real format exists per platform today (see README.md's own "nsis/rpm/appimage
   * aren't implemented yet") — a multi-select regardless, so a second format later doesn't
   * need reshaping this UI. */
  availableFormats: FormatOption[];
  formats: string[];
  onFormatsChange: (formats: string[]) => void;
}

export function InstallerCard({
  platform,
  title,
  typeLabel,
  enabled,
  onEnabledChange,
  availableFormats,
  formats,
  onFormatsChange,
}: InstallerCardProps) {
  const PlatformIcon = PLATFORM_ICONS[platform];
  const anchor = useComboboxAnchor();

  return (
    <Card className="flex-1">
      <CardHeader className="flex-row items-center justify-between gap-2">
        <CardTitle className="flex items-center gap-2">
          <PlatformIcon size={20} />
          {title}
        </CardTitle>
        <Checkbox
          checked={enabled}
          onCheckedChange={(checked) => onEnabledChange(checked === true)}
        />
      </CardHeader>
      {enabled && (
        <CardContent className="flex flex-col gap-1.5">
          <Label className="text-muted-foreground text-xs">{typeLabel}</Label>
          <Combobox
            multiple
            items={availableFormats}
            value={formats}
            onValueChange={(value) => onFormatsChange(value as string[])}
          >
            <ComboboxChips ref={anchor}>
              {formats.map((format) => (
                <ComboboxChip key={format}>.{format}</ComboboxChip>
              ))}
              <ComboboxChipsInput />
            </ComboboxChips>
            <ComboboxContent anchor={anchor}>
              <ComboboxEmpty />
              <ComboboxList>
                {(item: FormatOption) => (
                  <ComboboxItem value={item.value}>{item.label}</ComboboxItem>
                )}
              </ComboboxList>
            </ComboboxContent>
          </Combobox>
        </CardContent>
      )}
    </Card>
  );
}
