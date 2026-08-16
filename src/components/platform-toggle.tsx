import { AppleLogo, type Icon, LinuxLogo, WindowsLogo } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";

export type Platform = "windows" | "linux" | "macos";

const PLATFORM_ICONS: Record<Platform, Icon> = {
  windows: WindowsLogo,
  linux: LinuxLogo,
  macos: AppleLogo,
};

interface PlatformToggleProps {
  platform: Platform;
  label: string;
  selected: boolean;
  disabled?: boolean;
  onSelectedChange: (selected: boolean) => void;
}

/** A selectable, icon-labeled toggle for one platform — used instead of a plain checkbox
 * list so the platform is recognizable at a glance, not just by its text label. */
export function PlatformToggle({
  platform,
  label,
  selected,
  disabled,
  onSelectedChange,
}: PlatformToggleProps) {
  const PlatformIcon = PLATFORM_ICONS[platform];
  return (
    <button
      type="button"
      aria-pressed={selected}
      disabled={disabled}
      onClick={() => onSelectedChange(!selected)}
      className={cn(
        "flex flex-1 flex-col items-center gap-2 rounded-lg border px-4 py-3 text-sm transition-colors",
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
