import { AndroidLogo, type Icon } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";

// Only "android" today -- "ios" joins this union once that platform actually exists (see
// settings.ts's own MobileSettings comment on why app name/orientation are already modeled
// as shared across mobile platforms, not per-platform, in anticipation of that).
export type MobilePlatform = "android";

/** Exported so other mobile-specific UI uses the exact same icon per platform, mirroring
 * platform-toggle.tsx's own PLATFORM_ICONS for desktop. */
export const MOBILE_PLATFORM_ICONS: Record<MobilePlatform, Icon> = {
  android: AndroidLogo,
};

interface MobilePlatformToggleProps {
  platform: MobilePlatform;
  label: string;
  selected: boolean;
  disabled?: boolean;
  onSelectedChange: (selected: boolean) => void;
}

/** A selectable, icon-labeled toggle for one mobile platform -- the whole card is the
 * control (click anywhere to toggle), exactly mirroring platform-toggle.tsx's own
 * PlatformToggle for desktop/portable, rather than a separate checkbox. */
export function MobilePlatformToggle({
  platform,
  label,
  selected,
  disabled,
  onSelectedChange,
}: MobilePlatformToggleProps) {
  const PlatformIcon = MOBILE_PLATFORM_ICONS[platform];
  return (
    <button
      type="button"
      aria-pressed={selected}
      disabled={disabled}
      onClick={() => onSelectedChange(!selected)}
      className={cn(
        // Unlike platform-toggle.tsx's own PlatformToggle (flex-1, sized to share a row of
        // 3), this is normally the only card in its row -- fixed to roughly the same width
        // a Windows/Linux/macOS card ends up at, rather than stretching to fill the row.
        "flex w-full max-w-[200px] flex-col items-center gap-2 rounded-lg border px-4 py-3 text-sm transition-colors",
        "disabled:cursor-not-allowed disabled:opacity-50",
        selected
          ? "border-primary bg-primary/10 text-primary"
          : "border-input text-muted-foreground hover:bg-accent hover:text-accent-foreground",
      )}
    >
      <PlatformIcon size={28} weight={selected ? "fill" : "regular"} />
      {label}
    </button>
  );
}
