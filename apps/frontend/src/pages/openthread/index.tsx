import { Navigate, Route, Routes, useLocation, useNavigate } from 'react-router-dom';
import { Tab, Tabs } from '@blueprintjs/core';
import { ThreadSettings } from '../settings/thread';
import { NetworkScanner } from './network-scanner';
import { ThreadMesh } from './thread-mesh';
import './openthread.css';

type OpenThreadTabId = 'settings' | 'mesh' | 'scanner';

export function OpenThread() {
  const location = useLocation();
  const navigate = useNavigate();
  const selectedTab: OpenThreadTabId = location.pathname.endsWith('/mesh')
    ? 'mesh'
    : location.pathname.endsWith('/scanner')
      ? 'scanner'
      : 'settings';

  return (
    <div className="openthread-page">
      <nav className="openthread-tabs" aria-label="OpenThread sections">
        <Tabs
          id="openthread-tabs"
          selectedTabId={selectedTab}
          onChange={(tabId) => navigate(`/openthread/${String(tabId)}`)}
        >
          <Tab id="settings" title="OpenThread Settings" />
          <Tab id="mesh" title="OpenThread Mesh" />
          <Tab id="scanner" title="Network Scanner" />
        </Tabs>
      </nav>
      <main
        className={`openthread-content${selectedTab === 'settings' ? ' openthread-content--settings' : ''}`}
      >
        <Routes>
          <Route index element={<Navigate to="settings" replace />} />
          <Route path="settings" element={<ThreadSettings />} />
          <Route path="mesh" element={<ThreadMesh />} />
          <Route path="scanner" element={<NetworkScanner />} />
          <Route path="*" element={<Navigate to="settings" replace />} />
        </Routes>
      </main>
    </div>
  );
}
