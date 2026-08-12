import { Button } from "@blueprintjs/core";
import { SiteFooter, SiteShell } from "@extrittio/app-shell";
import { CtaBar, FeatureGrid, SiteHero, SiteSection } from "@extrittio/ui";
import type { SiteNavItem } from "@extrittio/navigation";
import { footerGroups, siteActions } from "../fixtures/site-nav";

const longNavItems: SiteNavItem[] = [
  {
    label: "Extremely long product navigation label",
    href: "/stress",
    children: [
      {
        label: "Long child route for stress testing",
        href: "/stress#child-one",
      },
      {
        label: "Another nested destination with a long name",
        href: "/stress#child-two",
      },
    ],
  },
  { label: "Documentation and implementation notes", href: "/stress#docs" },
  { label: "Gallery of customer-facing examples", href: "/stress#gallery" },
  { label: "Pricing", href: "/stress#pricing" },
  { label: "Company", href: "/stress#company" },
];

interface SiteRouteProps {
  currentPath: string;
  onNavigate: (href: string) => void;
}

export const StressPage = ({ currentPath, onNavigate }: SiteRouteProps) => (
  <SiteShell
    productName="Extrittio Long Brand Name"
    navItems={longNavItems}
    actions={siteActions}
    currentPath={currentPath}
    onNavigate={onNavigate}
    footer={
      <SiteFooter
        productName="Extrittio Long Brand Name"
        groups={footerGroups}
      />
    }
  >
    <SiteHero
      eyebrow="Stress route"
      title="Long labels, dense content, and responsive pressure"
      description="Use this route to catch text wrapping, mobile navigation behavior, and layout edge cases before adding a component to a product page."
      actions={
        <>
          <Button intent="primary" text="Primary action with long text" />
          <Button text="Secondary action" />
        </>
      }
    />
    <SiteSection title="Long feature copy">
      <FeatureGrid
        features={[
          {
            title: "A feature title that intentionally runs longer than usual",
            description:
              "This description is deliberately verbose so the card has to handle realistic content pressure across desktop and mobile widths.",
            icon: "paragraph",
            meta: "LONG",
          },
          {
            title: "Compact icon and metadata alignment",
            description:
              "Tests whether short cards and long cards can sit next to each other without creating awkward visual imbalance.",
            icon: "alignment-left",
            meta: "MIXED",
          },
          {
            title: "Repeated content block",
            description:
              "Useful for scanning border treatment, spacing, and hover-safe dimensions in repeated public-facing content.",
            icon: "duplicate",
            meta: "GRID",
          },
        ]}
      />
    </SiteSection>
    <SiteSection>
      <CtaBar
        title="CTA text that wraps gracefully instead of resizing the layout"
        description="This intentionally uses longer content to validate the responsive rules."
        actions={
          <Button
            intent="primary"
            text="Open product preview"
            onClick={() => onNavigate("/site/product")}
          />
        }
      />
    </SiteSection>
  </SiteShell>
);
