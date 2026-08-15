import { useTranslation } from "react-i18next";
import roversIcon from "@/assets/roves-icon.svg";

/**
 * Shown on every view (see this project's own CLAUDE.md) — the icon and the
 * "Roves" wordmark together, the same lockup the engine's own boot splash
 * uses, so this tool reads as unmistakably part of the same product.
 */
export function BrandHeader() {
  const { t } = useTranslation();

  return (
    <header className="flex items-center gap-3 px-6 py-4">
      <img src={roversIcon} alt="" className="h-9 w-9" />
      <span className="font-heading text-2xl leading-none tracking-wide">Roves</span>
      <span className="text-muted-foreground text-sm">{t("app.name")}</span>
    </header>
  );
}
