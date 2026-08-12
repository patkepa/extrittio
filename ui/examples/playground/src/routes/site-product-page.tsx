import { Button, Tag } from "@blueprintjs/core";
import { SiteFooter, SiteShell } from "@extrittio/app-shell";
import {
  CtaBar,
  DemoFrame,
  FeatureGrid,
  MetricStrip,
  SiteHero,
  SiteSection,
} from "@extrittio/ui";
import { footerGroups, siteActions, siteNavItems } from "../fixtures/site-nav";
import { productFeatures, productMetrics } from "../fixtures/site-content";
import { WorkspacePanelMock } from "../workspace-panel-mock";

interface SiteRouteProps {
  currentPath: string;
  onNavigate: (href: string) => void;
}

export const SiteProductPage = ({
  currentPath,
  onNavigate,
}: SiteRouteProps) => (
  <SiteShell
    productName="Extrittio"
    navItems={siteNavItems}
    actions={siteActions}
    currentPath={currentPath}
    onNavigate={onNavigate}
    footer={
      <SiteFooter productName="Extrittio" groups={footerGroups}>
        Public-site shell preview using the same tokens as the workspace
        package.
      </SiteFooter>
    }
  >
    <SiteHero
      id="demo"
      eyebrow="Product site preview"
      title="Operate every deployment from one workspace"
      description="A full public product page built from the same package primitives as the logged-in SaaS workspace."
      actions={
        <>
          <Button intent="primary" text="Request demo" />
          <Button
            minimal
            text="Open workspace preview"
            onClick={() => onNavigate("/workspace")}
          />
        </>
      }
      media={
        <DemoFrame
          title="Workspace demo panel"
          eyebrow="LIVE MOCK"
          actions={<Tag minimal>Product page asset</Tag>}
          footer="This frame is intentionally reusable for screenshots, videos, or interactive mocks."
        >
          <WorkspacePanelMock />
        </DemoFrame>
      }
    />

    <SiteSection
      eyebrow="System shape"
      title="Public pages without losing the workspace identity"
      description="Use these sections to tune hierarchy, spacing, color, and responsive behavior before moving patterns into product pages."
    >
      <MetricStrip metrics={productMetrics} />
    </SiteSection>

    <SiteSection
      id="capabilities"
      eyebrow="Capabilities"
      title="A reusable language for product storytelling"
      description="Feature cards are restrained by default so demos, docs, blogs, and galleries can share the same foundation."
    >
      <FeatureGrid features={productFeatures} />
    </SiteSection>

    <SiteSection>
      <CtaBar
        title="Ready to test a new public page?"
        description="Build it here first, validate the interaction and density, then promote the stable pieces back into packages."
        actions={
          <>
            <Button
              intent="primary"
              text="Component gallery"
              onClick={() => onNavigate("/components")}
            />
            <Button text="Stress cases" onClick={() => onNavigate("/stress")} />
          </>
        }
      />
    </SiteSection>
  </SiteShell>
);
