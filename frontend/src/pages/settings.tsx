import { Card, Elevation, H3, H5, FormGroup, InputGroup, Switch } from '@blueprintjs/core';
import './settings.css';

export const Settings = () => {
  return (
    <div className="settings-page">
      <div className="page-header">
        <H3>Settings</H3>
        <p className="page-description">Manage your application settings</p>
      </div>

      <div className="settings-content">
        <Card elevation={Elevation.TWO} className="settings-card">
          <H5>Profile Settings</H5>
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
        </Card>

        <Card elevation={Elevation.TWO} className="settings-card">
          <H5>Notifications</H5>
          <FormGroup>
            <Switch label="Email notifications" defaultChecked />
            <Switch label="Push notifications" />
            <Switch label="Device alerts" defaultChecked />
          </FormGroup>
        </Card>

        <Card elevation={Elevation.TWO} className="settings-card">
          <H5>Display</H5>
          <FormGroup>
            <Switch label="Compact mode" />
            <Switch label="Show device thumbnails" defaultChecked />
            <Switch label="Enable animations" defaultChecked />
          </FormGroup>
        </Card>
      </div>
    </div>
  );
};
