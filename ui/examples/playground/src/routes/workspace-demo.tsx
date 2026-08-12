import { useState } from "react";
import { Button, Card, Tag } from "@blueprintjs/core";
import { WorkspaceShell } from "@extrittio/app-shell";
import { SearchField, StatusLed, WorkspaceToolbar } from "@extrittio/ui";
import {
  workspaceNavGroups,
  workspaceProjects,
  workspaceUser,
} from "../fixtures/workspace-nav";

const rows = [
  {
    name: "North region rollout",
    status: "online",
    owner: "Field Ops",
    updated: "14:22",
  },
  {
    name: "Customer gateway migration",
    status: "warning",
    owner: "Success",
    updated: "13:58",
  },
  {
    name: "Backhaul audit",
    status: "online",
    owner: "Network",
    updated: "12:41",
  },
  {
    name: "Legacy route cleanup",
    status: "offline",
    owner: "Support",
    updated: "11:03",
  },
] as const;

export const WorkspaceDemo = () => {
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false);
  const [query, setQuery] = useState("");

  return (
    <WorkspaceShell
      productName="Extrittio Workspace"
      collapsedProductName="EW"
      navGroups={workspaceNavGroups}
      sidebarCollapsed={sidebarCollapsed}
      onToggleSidebar={() => setSidebarCollapsed((collapsed) => !collapsed)}
      breadcrumb={
        <>
          <span className="breadcrumb-link">Workspace</span>
          <span className="breadcrumb-sep">/</span>
          <span>Overview</span>
        </>
      }
      navBadges={{
        Deployments: { count: 12 },
        Fleet: { status: "online" },
        Alerts: { count: 3 },
      }}
      projects={workspaceProjects}
      user={workspaceUser}
      version="v0.1.0"
      onOpenCommandPalette={() => undefined}
    >
      <div className="workspace-demo-page">
        <WorkspaceToolbar ariaLabel="Workspace demo controls">
          <SearchField
            value={query}
            onChange={setQuery}
            placeholder="Search workspace preview"
          />
          <div className="workspace-toolbar-actions">
            <Button icon="filter" text="Filters" />
            <Button intent="primary" icon="add" text="New deployment" />
          </div>
        </WorkspaceToolbar>

        <section className="workspace-demo-summary">
          <Card>
            <span className="section-label">Active deployments</span>
            <strong className="workspace-demo-number mono-data">28</strong>
            <p>Live customer-facing initiatives across production.</p>
          </Card>
          <Card>
            <span className="section-label">Incidents</span>
            <strong className="workspace-demo-number mono-data">03</strong>
            <p>Warnings are visible but do not dominate the layout.</p>
          </Card>
          <Card>
            <span className="section-label">Signal quality</span>
            <strong className="workspace-demo-number mono-data">98.7%</strong>
            <p>Useful for testing numeric rhythm in cards.</p>
          </Card>
        </section>

        <section
          className="workspace-demo-table"
          aria-label="Workspace preview rows"
        >
          <div className="workspace-demo-table-header">
            <div>Workstream</div>
            <div>Status</div>
            <div>Owner</div>
            <div>Updated</div>
          </div>
          {rows.map((row) => (
            <div className="workspace-demo-row" key={row.name}>
              <div>
                <strong>{row.name}</strong>
                <span>Matches dense operational table styling.</span>
              </div>
              <div className="workspace-demo-status">
                <StatusLed status={row.status} />
                <Tag minimal>{row.status}</Tag>
              </div>
              <div>{row.owner}</div>
              <div className="mono-data">{row.updated}</div>
            </div>
          ))}
        </section>
      </div>
    </WorkspaceShell>
  );
};
