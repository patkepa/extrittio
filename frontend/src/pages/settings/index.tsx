import { Routes, Route, Navigate } from 'react-router-dom';
import { ProfileSettings } from './profile';
import { DeviceTypesSettings } from './device-types';
import { FleetsSettings } from './fleets';

export const Settings = () => (
  <Routes>
    <Route index element={<Navigate to="profile" replace />} />
    <Route path="profile" element={<ProfileSettings />} />
    <Route path="device-types" element={<DeviceTypesSettings />} />
    <Route path="fleets" element={<FleetsSettings />} />
    <Route path="*" element={<Navigate to="profile" replace />} />
  </Routes>
);
