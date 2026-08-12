import { StatusLed } from "@extrittio/ui";

const panelRows = [
  ["Edge router rollout", "online", "98%"],
  ["Customer sync queue", "warning", "12"],
  ["Billing export", "online", "04m"],
  ["Legacy tunnel audit", "offline", "Paused"],
] as const;

interface WorkspacePanelMockProps {
  compact?: boolean;
}

export const WorkspacePanelMock = ({
  compact = false,
}: WorkspacePanelMockProps) => (
  <div
    className={[
      "mock-workspace-panel",
      compact && "mock-workspace-panel--compact",
    ]
      .filter(Boolean)
      .join(" ")}
  >
    <div className="mock-panel-sidebar">
      <span />
      <span />
      <span />
      <span />
    </div>
    <div className="mock-panel-main">
      <div className="mock-panel-header">
        <div>
          <span className="mono-data">OPS / LIVE</span>
          <strong>Deployment overview</strong>
        </div>
        <span className="mock-panel-pill">Production</span>
      </div>
      <div className="mock-panel-grid">
        <div>
          <span>Active</span>
          <strong className="mono-data">28</strong>
        </div>
        <div>
          <span>Warnings</span>
          <strong className="mono-data">03</strong>
        </div>
        <div>
          <span>SLA</span>
          <strong className="mono-data">99.9</strong>
        </div>
      </div>
      <div className="mock-panel-rows">
        {panelRows.map(([label, status, value]) => (
          <div className="mock-panel-row" key={label}>
            <StatusLed status={status} />
            <span>{label}</span>
            <strong className="mono-data">{value}</strong>
          </div>
        ))}
      </div>
    </div>
  </div>
);
