import { Routes, Route, Navigate } from 'react-router-dom';
import { ProfileSettings } from './profile';
import { DeviceTypesSettings } from './device-types';
import { FleetsSettings } from './fleets';
import { UsersSettings } from './users';
import { RolesSettings } from './roles';
import { FirmwareSettings } from './firmware';
import { CertificatesSettings } from './certificates';
import { ApiKeysSettings } from './api-keys';
import { getAccessibleSettingsRoutes, getDefaultSettingsPath } from '../../app/routes';
import { useAuthStore } from '../../stores/auth-store';

const settingsElements = {
  profile: <ProfileSettings />,
  'device-types': <DeviceTypesSettings />,
  fleets: <FleetsSettings />,
  users: <UsersSettings />,
  roles: <RolesSettings />,
  firmware: <FirmwareSettings />,
  certificates: <CertificatesSettings />,
  'api-keys': <ApiKeysSettings />,
} as const;

export const Settings = () => {
  const permissions = useAuthStore((s) => s.user?.permissions);
  const routes = getAccessibleSettingsRoutes(permissions);
  const defaultPath = getDefaultSettingsPath(permissions);

  return (
    <Routes>
      <Route index element={<Navigate to={defaultPath} replace />} />
      {routes.map((route) => (
        <Route
          key={route.id}
          path={route.path}
          element={settingsElements[route.id as keyof typeof settingsElements]}
        />
      ))}
      <Route path="*" element={<Navigate to={defaultPath} replace />} />
    </Routes>
  );
};
