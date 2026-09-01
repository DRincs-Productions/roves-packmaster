import { PLATFORM_ICONS, type Platform } from "@/components/platform-toggle";
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
import { cn } from "@/lib/utils";

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
  /** Real feasibility (right host OS + native tool actually installed) — see
   * src/lib/installer-availability.ts. `null` while still checking. */
  available: boolean | null;
  unavailableReason: string | null;
}

/** The toggle itself is a whole clickable card, mirroring platform-toggle.tsx's own
 * PlatformToggle (used for the "Portable" section above) rather than a separate checkbox --
 * same look, same click-anywhere-on-the-card interaction, for both sections. The format
 * picker (only relevant once enabled) sits below as its own, separately-interactive area. */
export function InstallerCard({
  platform,
  title,
  typeLabel,
  enabled,
  onEnabledChange,
  availableFormats,
  formats,
  onFormatsChange,
  available,
  unavailableReason,
}: InstallerCardProps) {
  const PlatformIcon = PLATFORM_ICONS[platform];
  const anchor = useComboboxAnchor();
  const isAvailable = available ?? true;
  const selected = isAvailable && enabled;

  return (
    <div className="flex flex-1 flex-col gap-2">
      <button
        type="button"
        aria-pressed={selected}
        disabled={!isAvailable}
        onClick={() => onEnabledChange(!enabled)}
        className={cn(
          "flex w-full flex-1 flex-col items-center gap-2 rounded-lg border px-4 py-3 text-sm transition-colors",
          "disabled:cursor-not-allowed disabled:opacity-50",
          selected
            ? "border-primary bg-primary/10 text-primary"
            : "border-input text-muted-foreground hover:bg-accent hover:text-accent-foreground",
        )}
      >
        <PlatformIcon size={28} weight={selected ? "fill" : "regular"} />
        {title}
      </button>
      {!isAvailable && unavailableReason && (
        <p className="text-muted-foreground text-xs">{unavailableReason}</p>
      )}
      {selected && (
        <div className="flex flex-col gap-1.5 rounded-lg border px-3 py-2.5">
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
        </div>
      )}
    </div>
  );
}
