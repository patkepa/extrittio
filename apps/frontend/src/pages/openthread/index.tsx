import { Navigate, Route, Routes, useLocation } from 'react-router-dom';
import { ThreadSettings } from '../settings/thread';
import { NetworkScanner } from './network-scanner';
import { ThreadMesh } from './thread-mesh';
import './openthread.css';

export function OpenThread() {
  const location = useLocation();
  const isSettings = location.pathname.endsWith('/settings');

  return (
    <div className="openthread-page">
      <main className={`openthread-content${isSettings ? ' openthread-content--settings' : ''}`}>
        <Routes>
          <Route index element={<Navigate to="scanner" replace />} />
          <Route path="settings" element={<ThreadSettings />} />
          <Route path="mesh" element={<ThreadMesh />} />
          <Route path="scanner" element={<NetworkScanner />} />
          <Route path="*" element={<Navigate to="scanner" replace />} />
        </Routes>
      </main>
    </div>
  );
}
