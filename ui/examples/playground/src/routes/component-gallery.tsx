import { Button, Tag } from "@blueprintjs/core";
import { SiteFooter, SiteShell } from "@extrittio/app-shell";
import {
  CtaBar,
  DemoFrame,
  FeatureGrid,
  MetricStrip,
  SiteGrid,
  SiteHero,
  SiteSection,
} from "@extrittio/ui";
import { productFeatures, productMetrics } from "../fixtures/site-content";
import { footerGroups, siteActions, siteNavItems } from "../fixtures/site-nav";
import { WorkspacePanelMock } from "../workspace-panel-mock";

interface SiteRouteProps {
  currentPath: string;
  onNavigate: (href: string) => void;
}

export const ComponentGallery = ({
  currentPath,
  onNavigate,
}: SiteRouteProps) => (
  <SiteShell
    productName="Extrittio"
    navItems={siteNavItems}
    actions={siteActions}
    currentPath={currentPath}
    onNavigate={onNavigate}
    footer={<SiteFooter productName="Extrittio" groups={footerGroups} />}
  >
    <SiteHero
      eyebrow="Component gallery"
      title="Reusable blocks for site pages"
      description="A focused route for validating public-page primitives without committing to one marketing page composition."
    />
    <SiteSection title="Grid primitives">
      <SiteGrid>
        <DemoFrame title="Demo frame" eyebrow="FRAME" footer="Footer slot">
          <WorkspacePanelMock compact />
        </DemoFrame>
        <CtaBar
          title="CTA bar"
          description="Useful for product, docs, and campaign pages."
          actions={<Button text="Action" />}
        />
      </SiteGrid>
    </SiteSection>
    <SiteSection title="Feature grid">
      <FeatureGrid features={productFeatures} />
    </SiteSection>
    <SiteSection title="Metrics" actions={<Tag minimal>Tabular values</Tag>}>
      <MetricStrip metrics={productMetrics} />
    </SiteSection>
  </SiteShell>
);
