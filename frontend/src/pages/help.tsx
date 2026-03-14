import { Card, Elevation, H3, AnchorButton } from '@blueprintjs/core';

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
          <span className="section-label">Source Code</span>
          <p style={{ marginTop: 12, marginBottom: 16 }}>
            Extrittio is open-source. View the source code, report issues, or
            contribute on GitHub.
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
