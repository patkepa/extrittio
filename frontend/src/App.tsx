import { lazy, Suspense } from 'react';
import { BrowserRouter, Routes, Route, Navigate } from 'react-router-dom';
import { Spinner } from '@blueprintjs/core';
import { ThemeProvider } from './context/theme-provider';
import { MainLayout } from './components/layout/main-layout';
import { AuthGuard } from './components/auth-guard';
import { Login } from './pages/login';

// Lazy-loaded pages — split into separate chunks
const Dashboard = lazy(() => import('./pages/dashboard').then((m) => ({ default: m.Dashboard })));
const Devices = lazy(() => import('./pages/devices').then((m) => ({ default: m.Devices })));
const DeviceDetail = lazy(() =>
  import('./pages/device-detail').then((m) => ({ default: m.DeviceDetail })),
);
const FleetGraph = lazy(() =>
  import('./pages/fleet-graph').then((m) => ({ default: m.FleetGraph })),
);
const Settings = lazy(() =>
  import('./pages/settings/index').then((m) => ({ default: m.Settings })),
);
const Help = lazy(() => import('./pages/help').then((m) => ({ default: m.Help })));
const Rules = lazy(() => import('./pages/rules').then((m) => ({ default: m.Rules })));
const AlertsPage = lazy(() => import('./pages/alerts').then((m) => ({ default: m.Alerts })));

// Import Blueprint.js styles
import '@blueprintjs/core/lib/css/blueprint.css';
import '@blueprintjs/icons/lib/css/blueprint-icons.css';
import './styles/theme.css';

const PageFallback = () => (
  <div style={{ display: 'flex', justifyContent: 'center', alignItems: 'center', height: '100%' }}>
    <Spinner size={40} />
  </div>
);

function App() {
  return (
    <ThemeProvider>
      <BrowserRouter>
        <Routes>
          <Route path="/login" element={<Login />} />
          <Route
            path="/*"
            element={
              <AuthGuard>
                <MainLayout>
                  <Suspense fallback={<PageFallback />}>
                    <Routes>
                      <Route path="/" element={<Dashboard />} />
                      <Route path="/devices" element={<Devices />} />
                      <Route path="/devices/:deviceId" element={<DeviceDetail />} />
                      <Route path="/fleet-graph" element={<FleetGraph />} />
                      <Route path="/rules" element={<Rules />} />
                      <Route path="/alerts" element={<AlertsPage />} />
                      <Route path="/help" element={<Help />} />
                      <Route path="/settings/*" element={<Settings />} />
                      <Route path="*" element={<Navigate to="/" replace />} />
                    </Routes>
                  </Suspense>
                </MainLayout>
              </AuthGuard>
            }
          />
        </Routes>
      </BrowserRouter>
    </ThemeProvider>
  );
}

export default App;
