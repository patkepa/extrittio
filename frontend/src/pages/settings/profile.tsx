import { Card, Elevation, H3, FormGroup, InputGroup } from '@blueprintjs/core';
import './settings.css';

export const ProfileSettings = () => {
  return (
    <div className="settings-page">
      <div className="page-header">
        <div>
          <H3>Profile</H3>
          <p className="page-description">Manage your profile information</p>
        </div>
      </div>

      <div className="settings-content">
        <Card elevation={Elevation.ONE} className="settings-card">
          <span className="section-label">Personal Information</span>
          <div className="settings-form">
            <FormGroup label="Name" labelFor="name-input">
              <InputGroup id="name-input" placeholder="Enter your name" />
            </FormGroup>
            <FormGroup label="Email" labelFor="email-input">
              <InputGroup id="email-input" type="email" placeholder="Enter your email" />
            </FormGroup>
          </div>
        </Card>
      </div>
    </div>
  );
};
