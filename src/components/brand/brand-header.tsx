import roversIcon from "@/assets/roves-icon.svg";

/**
 * Shown on every view (see this project's own CLAUDE.md) — the icon and the
 * "Roves Packmaster" wordmark together, in the engine's own boot-splash font
 * (Metal Mania, `font-heading`), so this tool reads as unmistakably part of
 * the same product.
 */
export function BrandHeader() {
  return (
    <header className="flex items-center gap-3 px-6 py-4">
      <img src={roversIcon} alt="" className="h-9 w-9" />
      <span className="font-heading text-2xl leading-none tracking-wide">Roves Packmaster</span>
    </header>
  );
}
