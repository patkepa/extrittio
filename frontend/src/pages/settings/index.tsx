import { Routes, Route, Navigate } from 'react-router-dom';
import { DeviceTypesSettings } from './device-types';
import { FleetsSettings } from './fleets';

export const Settings = () => (
  <Routes>
    <Route index element={<Navigate to="device-types" replace />} />
    <Route path="device-types" element={<DeviceTypesSettings />} />
    <Route path="fleets" element={<FleetsSettings />} />
    <Route path="*" element={<Navigate to="device-types" replace />} />
  </Routes>
);
