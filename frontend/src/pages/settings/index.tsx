import { Routes, Route, Navigate } from 'react-router-dom';
import { ProfileSettings } from './profile';
import { DeviceTypesSettings } from './device-types';
import { FleetsSettings } from './fleets';
import { UsersSettings } from './users';
import { FirmwareSettings } from './firmware';
import { CertificatesSettings } from './certificates';
import { ApiKeysSettings } from './api-keys';
import { settingsRoutes } from '../../app/routes';

const settingsElements = {
  profile: <ProfileSettings />,
  'device-types': <DeviceTypesSettings />,
  fleets: <FleetsSettings />,
  users: <UsersSettings />,
  firmware: <FirmwareSettings />,
  certificates: <CertificatesSettings />,
  'api-keys': <ApiKeysSettings />,
} as const;

export const Settings = () => (
  <Routes>
    <Route index element={<Navigate to="profile" replace />} />
    {settingsRoutes.map((route) => (
      <Route
        key={route.id}
        path={route.path}
        element={settingsElements[route.id as keyof typeof settingsElements]}
      />
    ))}
    <Route path="*" element={<Navigate to="profile" replace />} />
  </Routes>
);
