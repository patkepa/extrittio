import { BrowserRouter, Routes, Route, Navigate } from 'react-router-dom';
import { ThemeProvider } from './context/theme-provider';
import { TooltipProvider } from './components/ui/tooltip';
import { MainLayout } from './components/layout/main-layout';
import { Dashboard } from './pages/dashboard';
import { Devices } from './pages/devices';
import { Settings } from './pages/settings';
import { Users } from './pages/users';
import { HelpCenter } from './pages/help-center';
import './app.css';
import './styles/theme.css';

function App() {
  return (
    <ThemeProvider>
      <TooltipProvider>
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
      </TooltipProvider>
    </ThemeProvider>
  );
}

export default App;
