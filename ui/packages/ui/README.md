# @extrittio/ui

Reusable UI primitives shared by the Extrittio workspace and public site surfaces.

## Exports

- `WorkspaceToolbar`
- `WorkspaceBottomToolbar`
- `MainToolbar`
- `BottomToolbar`
- `SiteSection`
- `SiteHero`
- `SiteGrid`
- `FeatureGrid`
- `MetricStrip`
- `DemoFrame`
- `CtaBar`
- `StatusLed`
- `SearchField`
- `FilterPill`
- `EmptyState`
- `@extrittio/ui/styles.css`

## Workspace Usage

```tsx
import { EmptyState, SearchField, StatusLed } from "@extrittio/ui";
import "@extrittio/ui/styles.css";

export function DeviceSearch() {
  return (
    <>
      <SearchField
        value=""
        onChange={() => undefined}
        placeholder="Search devices"
      />
      <StatusLed status="online" label="Online" />
      <EmptyState icon="search" title="No devices found" />
    </>
  );
}
```

## Site Usage

```tsx
import { CtaBar, FeatureGrid, SiteHero, SiteSection } from "@extrittio/ui";
import "@extrittio/ui/styles.css";

export function ProductPage() {
  return (
    <>
      <SiteHero
        title="Operate every deployment from one workspace"
        description="A public product page can reuse the same dark technical design language without using the workspace sidebar."
      />
      <SiteSection title="Capabilities">
        <FeatureGrid
          features={[
            {
              title: "Live operations",
              description:
                "Track active systems, incidents, and delivery state.",
              icon: "pulse",
            },
          ]}
        />
      </SiteSection>
      <CtaBar title="See the workspace in action" />
    </>
  );
}
```
