import { Card, Elevation, H3, FormGroup, InputGroup, Switch } from '@blueprintjs/core';
import './settings.css';

export const SettingsHome = () => {
  return (
    <div className="settings-page">
      <div className="page-header">
        <H3>Settings</H3>
        <p className="page-description">Application configuration</p>
      </div>

      <div className="settings-content">
        <Card elevation={Elevation.TWO} className="settings-card">
          <span className="section-label">Profile</span>
          <div className="settings-form">
            <FormGroup label="Name" labelFor="name-input">
              <InputGroup id="name-input" placeholder="Enter your name" defaultValue="satnaing" />
            </FormGroup>
            <FormGroup label="Email" labelFor="email-input">
              <InputGroup
                id="email-input"
                type="email"
                placeholder="Enter your email"
                defaultValue="satnaingdev@gmail.com"
              />
            </FormGroup>
          </div>
        </Card>

        <Card elevation={Elevation.TWO} className="settings-card">
          <span className="section-label">Notifications</span>
          <div className="settings-switches">
            <Switch label="Email notifications" defaultChecked />
            <Switch label="Push notifications" />
            <Switch label="Device alerts" defaultChecked />
          </div>
        </Card>

        <Card elevation={Elevation.TWO} className="settings-card">
          <span className="section-label">Display</span>
          <div className="settings-switches">
            <Switch label="Compact mode" />
            <Switch label="Show device thumbnails" defaultChecked />
            <Switch label="Enable animations" defaultChecked />
          </div>
        </Card>
      </div>
    </div>
  );
};
