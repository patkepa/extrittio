import { AnchorButton, Card, Elevation, H3, HTMLTable, Tag } from '@blueprintjs/core';

const shortcuts = [
  ['Cmd/Ctrl + K', 'Open command palette'],
  ['Cmd/Ctrl + B', 'Toggle sidebar'],
  ['/', 'Focus device search'],
  ['W/S or ↑/↓', 'Move through device rows and form controls'],
  ['A/D or ←/→', 'Move between device detail tabs'],
  ['Enter', 'Open the focused device row'],
  ['Space', 'Select the focused device row'],
  ['Home/End', 'Jump to the first or last row/control'],
  ['Esc or B', 'Leave a device detail page'],
];

export const Help = () => {
  return (
    <div className="settings-page">
      <div className="page-header">
        <div>
          <H3>Help</H3>
          <p className="page-description">Resources and support</p>
        </div>
      </div>

      <div className="settings-content">
        <Card elevation={Elevation.ONE} className="settings-card">
          <span className="section-label">Keyboard Shortcuts</span>
          <HTMLTable compact className="tab-table" style={{ marginTop: 12 }}>
            <tbody>
              {shortcuts.map(([keys, action]) => (
                <tr key={keys}>
                  <td>
                    <Tag minimal className="mono-data">
                      {keys}
                    </Tag>
                  </td>
                  <td>{action}</td>
                </tr>
              ))}
            </tbody>
          </HTMLTable>
        </Card>

        <Card elevation={Elevation.ONE} className="settings-card">
          <span className="section-label">Source Code</span>
          <p style={{ marginTop: 12, marginBottom: 16 }}>
            Extrittio is open-source. View the source code, report issues, or contribute on GitHub.
          </p>
          <AnchorButton
            icon="git-repo"
            href="https://github.com/patkepa/extrittio"
            target="_blank"
            rel="noopener noreferrer"
            intent="primary"
          >
            View on GitHub
          </AnchorButton>
        </Card>
      </div>
    </div>
  );
};
