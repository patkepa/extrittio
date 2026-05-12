import { Suspense } from 'react';
import { BrowserRouter, Routes, Route, Navigate } from 'react-router-dom';
import { Spinner } from '@blueprintjs/core';
import { ThemeProvider } from '@extrittio/theme';
import { ExtrittioShell } from './app/extrittio-shell';
import { AuthGuard } from './components/auth-guard';
import { Login } from './pages/login';
import { protectedRoutes } from './app/routes';

// Import Blueprint.js styles
import '@blueprintjs/core/lib/css/blueprint.css';
import '@blueprintjs/icons/lib/css/blueprint-icons.css';
import '@extrittio/theme/theme.css';

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
                <ExtrittioShell>
                  <Suspense fallback={<PageFallback />}>
                    <Routes>
                      {protectedRoutes.map((route) => (
                        <Route key={route.id} path={route.path} element={route.element} />
                      ))}
                      <Route path="*" element={<Navigate to="/" replace />} />
                    </Routes>
                  </Suspense>
                </ExtrittioShell>
              </AuthGuard>
            }
          />
        </Routes>
      </BrowserRouter>
    </ThemeProvider>
  );
}

export default App;
