import { createRootRoute, Outlet } from "@tanstack/react-router";
import { BrandHeader } from "@/components/brand/brand-header";
import { InfoPopover } from "@/components/brand/info-popover";
import { LanguageSwitcher } from "@/components/brand/language-switcher";

export const Route = createRootRoute({
  component: RootLayout,
});

// Every view shows the brand header (icon + logo — see this project's own
// CLAUDE.md) via this shared layout, so no individual route needs to
// remember to render it itself.
function RootLayout() {
  return (
    <div className="flex min-h-svh flex-col">
      <div className="bg-background sticky top-0 z-20 flex items-center justify-between border-b">
        <BrandHeader />
        <div className="flex items-center gap-2 pr-6">
          <InfoPopover />
          <LanguageSwitcher />
        </div>
      </div>
      <main className="flex flex-1 flex-col items-center px-6 py-10">
        <Outlet />
      </main>
    </div>
  );
}
