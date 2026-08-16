import { createRootRoute, Outlet } from "@tanstack/react-router";
import { BrandHeader } from "@/components/brand/brand-header";
import { LanguageSwitcher } from "@/components/brand/language-switcher";
import { ShellUpdateBanner } from "@/components/shell-update-banner";

export const Route = createRootRoute({
  component: RootLayout,
});

// Every view shows the brand header (icon + logo — see this project's own
// CLAUDE.md) via this shared layout, so no individual route needs to
// remember to render it itself.
function RootLayout() {
  return (
    <div className="flex min-h-svh flex-col">
      <div className="flex items-center justify-between border-b">
        <BrandHeader />
        <div className="pr-6">
          <LanguageSwitcher />
        </div>
      </div>
      <ShellUpdateBanner />
      <main className="flex flex-1 flex-col items-center justify-center px-6 py-10">
        <Outlet />
      </main>
    </div>
  );
}
