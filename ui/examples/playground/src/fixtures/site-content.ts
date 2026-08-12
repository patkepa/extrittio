import type { FeatureGridItem, MetricStripItem } from "@extrittio/ui";

export const productFeatures: FeatureGridItem[] = [
  {
    title: "Live operations",
    description:
      "Track active systems, incidents, and field state without leaving the workspace.",
    icon: "pulse",
    meta: "REALTIME",
  },
  {
    title: "Deployment control",
    description:
      "Coordinate rollout status, ownership, and customer-visible milestones from one surface.",
    icon: "git-branch",
    meta: "CONTROL",
  },
  {
    title: "Shared context",
    description:
      "Keep support, engineering, and customer success aligned around the same operating view.",
    icon: "diagram-tree",
    meta: "SYNC",
  },
];

export const productMetrics: MetricStripItem[] = [
  {
    label: "Regions",
    value: "14",
    description: "Modeled for global rollout views.",
  },
  {
    label: "Signals",
    value: "2.8M",
    description: "Dense status previews and telemetry rows.",
  },
  {
    label: "Response",
    value: "42ms",
    description: "Representative product stat formatting.",
  },
  {
    label: "Uptime",
    value: "99.99%",
    description: "Long-form values and tabular numbers.",
  },
];

export const articleCards = [
  {
    title: "Designing an operations workspace for repeated daily use",
    meta: "Product / 8 min",
    description:
      "A content layout test for dense writing, link treatment, and public-site rhythm.",
  },
  {
    title: "How customer-facing teams read deployment state",
    meta: "Field notes / 6 min",
    description:
      "Checks card density, metadata, and paragraph color against the dark theme.",
  },
  {
    title: "Building public pages from workspace design tokens",
    meta: "Engineering / 5 min",
    description:
      "Verifies the site surfaces can reuse the system without looking like an admin screen.",
  },
];
