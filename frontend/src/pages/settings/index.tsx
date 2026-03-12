import { Routes, Route, Navigate } from 'react-router-dom';
import { ProfileSettings } from './profile';
import { DeviceTypesSettings } from './device-types';
import { FleetsSettings } from './fleets';
import { UsersSettings } from './users';
import { FirmwareSettings } from './firmware';
import { CertificatesSettings } from './certificates';

export const Settings = () => (
  <Routes>
    <Route index element={<Navigate to="profile" replace />} />
    <Route path="profile" element={<ProfileSettings />} />
    <Route path="device-types" element={<DeviceTypesSettings />} />
    <Route path="fleets" element={<FleetsSettings />} />
    <Route path="users" element={<UsersSettings />} />
    <Route path="firmware" element={<FirmwareSettings />} />
    <Route path="certificates" element={<CertificatesSettings />} />
    <Route path="*" element={<Navigate to="profile" replace />} />
  </Routes>
);
