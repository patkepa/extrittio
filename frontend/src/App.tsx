import { BrowserRouter, Routes, Route, Navigate } from 'react-router-dom';
import { ThemeProvider } from './context/theme-provider';
import { MainLayout } from './components/layout/main-layout';
import { Dashboard } from './pages/dashboard';
import { Devices } from './pages/devices';
import { Settings } from './pages/settings/index';
import { Users } from './pages/users';
import { HelpCenter } from './pages/help-center';

// Import Blueprint.js styles
import '@blueprintjs/core/lib/css/blueprint.css';
import '@blueprintjs/icons/lib/css/blueprint-icons.css';
import './styles/theme.css';

function App() {
  return (
    <ThemeProvider>
      <BrowserRouter>
        <MainLayout>
          <Routes>
            <Route path="/" element={<Dashboard />} />
            <Route path="/devices" element={<Devices />} />
            <Route path="/settings/*" element={<Settings />} />
            <Route path="/users" element={<Users />} />
            <Route path="/help-center" element={<HelpCenter />} />
            <Route path="*" element={<Navigate to="/" replace />} />
          </Routes>
        </MainLayout>
      </BrowserRouter>
    </ThemeProvider>
  );
}

export default App;
