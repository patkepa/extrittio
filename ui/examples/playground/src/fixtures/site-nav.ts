import type {
  SiteNavAction,
  SiteNavGroup,
  SiteNavItem,
} from "@extrittio/navigation";

export const siteNavItems: SiteNavItem[] = [
  {
    label: "Product",
    href: "/site/product",
    children: [
      {
        label: "Platform",
        href: "/site/product",
        children: [
          {
            label: "Overview",
            href: "/site/product",
            description: "Full public page composition",
          },
          {
            label: "Demo frame",
            href: "/site/product#demo",
            description: "Workspace panel preview surface",
          },
          {
            label: "Capabilities",
            href: "/site/product#capabilities",
            description: "Feature cards and metrics",
          },
        ],
      },
      {
        label: "System",
        href: "/components",
        children: [
          {
            label: "Components",
            href: "/components",
            description: "Reusable site and workspace primitives",
          },
          {
            label: "Stress cases",
            href: "/stress",
            description: "Density, overflow, and state coverage",
          },
          {
            label: "Workspace",
            href: "/workspace",
            description: "Logged-in shell and navigation patterns",
          },
        ],
      },
      {
        label: "Resources",
        href: "/site/blog",
        children: [
          {
            label: "Blog",
            href: "/site/blog",
            description: "Editorial layout and article surfaces",
          },
          {
            label: "Request demo",
            href: "/site/product#demo",
            description: "Primary conversion path",
          },
          {
            label: "Sign in",
            href: "/workspace",
            description: "Open the workspace preview",
          },
        ],
      },
    ],
  },
  { label: "Blog", href: "/site/blog" },
  { label: "Components", href: "/components" },
  { label: "Stress", href: "/stress" },
];

export const siteActions: SiteNavAction[] = [
  { label: "Sign in", href: "/workspace" },
  { label: "Request demo", href: "/site/product#demo", intent: "primary" },
];

export const footerGroups: SiteNavGroup[] = [
  {
    label: "Product",
    items: [
      { label: "Workspace", href: "/workspace" },
      { label: "Demo", href: "/site/product#demo" },
      { label: "Capabilities", href: "/site/product#capabilities" },
    ],
  },
  {
    label: "Resources",
    items: [
      { label: "Blog", href: "/site/blog" },
      { label: "Components", href: "/components" },
      { label: "Stress cases", href: "/stress" },
    ],
  },
];
