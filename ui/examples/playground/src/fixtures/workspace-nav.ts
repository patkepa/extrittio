import type { NavGroup, Project, User } from "@extrittio/navigation";

export const workspaceNavGroups: NavGroup[] = [
  {
    label: "Operate",
    items: [
      { label: "Overview", icon: "dashboard", href: "/workspace" },
      {
        label: "Deployments",
        icon: "cloud-upload",
        href: "/workspace/deployments",
      },
      {
        label: "Fleet",
        icon: "layers",
        href: "/workspace/fleet",
        children: [
          {
            label: "Routers",
            icon: "route",
            href: "/workspace/fleet/routers",
          },
          {
            label: "Interfaces",
            icon: "exchange",
            href: "/workspace/fleet/interfaces",
          },
          {
            label: "Alerts",
            icon: "warning-sign",
            href: "/workspace/fleet/alerts",
          },
        ],
      },
    ],
  },
  {
    label: "Manage",
    items: [
      { label: "Customers", icon: "people", href: "/workspace/customers" },
      {
        label: "Reports",
        icon: "timeline-area-chart",
        href: "/workspace/reports",
      },
      { label: "Settings", icon: "cog", href: "/workspace/settings" },
    ],
  },
];

export const workspaceProjects: Project[] = [
  { name: "Production", environment: "Production", icon: "cloud" },
  { name: "Testing", environment: "Testing", icon: "lab-test" },
  { name: "Development", environment: "Development", icon: "code" },
];

export const workspaceUser: User = {
  name: "Avery Quinn",
  email: "avery@example.com",
};
