import { Routes, Route } from 'react-router-dom';
import { SettingsHome } from './home';
import { DeviceTypesSettings } from './device-types';
import { FleetsSettings } from './fleets';

export const Settings = () => (
  <Routes>
    <Route index element={<SettingsHome />} />
    <Route path="profile" element={<SettingsHome />} />
    <Route path="account" element={<SettingsHome />} />
    <Route path="appearance" element={<SettingsHome />} />
    <Route path="notifications" element={<SettingsHome />} />
    <Route path="display" element={<SettingsHome />} />
    <Route path="device-types" element={<DeviceTypesSettings />} />
    <Route path="fleets" element={<FleetsSettings />} />
    <Route path="*" element={<SettingsHome />} />
  </Routes>
);
