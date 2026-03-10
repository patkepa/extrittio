import { BrowserRouter, Routes, Route, Navigate } from 'react-router-dom';
import { ThemeProvider } from './context/theme-provider';
import { MainLayout } from './components/layout/main-layout';
import { AuthGuard } from './components/auth-guard';
import { Login } from './pages/login';
import { Dashboard } from './pages/dashboard';
import { Devices } from './pages/devices';
import { DeviceDetail } from './pages/device-detail';
import { Settings } from './pages/settings/index';

// Import Blueprint.js styles
import '@blueprintjs/core/lib/css/blueprint.css';
import '@blueprintjs/icons/lib/css/blueprint-icons.css';
import './styles/theme.css';

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
                  <Routes>
                    <Route path="/" element={<Dashboard />} />
                    <Route path="/devices" element={<Devices />} />
                    <Route path="/devices/:deviceId" element={<DeviceDetail />} />
                    <Route path="/settings/*" element={<Settings />} />
                    <Route path="*" element={<Navigate to="/" replace />} />
                  </Routes>
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
