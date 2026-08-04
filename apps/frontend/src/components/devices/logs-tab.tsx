import { useState } from 'react';
import { Callout, HTMLSelect, Spinner, Tag } from '@blueprintjs/core';
import { useDeviceLogs } from '../../hooks/use-logs';

interface LogsTabProps {
  deviceId: string;
}

const LEVEL_OPTIONS = [
  { label: 'All Levels', value: '' },
  { label: 'DEBUG', value: 'DEBUG' },
  { label: 'INFO', value: 'INFO' },
  { label: 'WARN', value: 'WARN' },
  { label: 'ERROR', value: 'ERROR' },
];

const LEVEL_INTENT: Record<string, 'none' | 'primary' | 'warning' | 'danger' | 'success'> = {
  DEBUG: 'none',
  INFO: 'primary',
  WARN: 'warning',
  ERROR: 'danger',
};

export const LogsTab = ({ deviceId }: LogsTabProps) => {
  const [levelFilter, setLevelFilter] = useState('');
  const params = {
    limit: 200,
    ...(levelFilter ? { level: levelFilter } : {}),
  };
  const { data: logs = [], isLoading, isError } = useDeviceLogs(deviceId, params);

  if (isLoading) return <Spinner />;

  if (isError) {
    return (
      <Callout intent="danger" icon="error">
        Failed to load logs. Try refreshing the page.
      </Callout>
    );
  }

  return (
    <div className="logs-tab">
      <div className="logs-toolbar">
        <HTMLSelect
          value={levelFilter}
          onChange={(e) => setLevelFilter(e.target.value)}
          options={LEVEL_OPTIONS}
          minimal
        />
        <span className="mono-data tab-cell-muted">{logs.length} entries</span>
      </div>

      {logs.length === 0 ? (
        <Callout icon="info-sign" intent="primary" className="tab-callout">
          No log entries found{levelFilter ? ` for level ${levelFilter}` : ''}.
        </Callout>
      ) : (
        <div className="logs-list">
          {logs.map((entry) => {
            const time = new Date(entry.created_at).toLocaleTimeString('en-GB', {
              hour: '2-digit',
              minute: '2-digit',
              second: '2-digit',
            });
            return (
              <div key={entry.id} className="log-entry">
                <span className="log-time mono-data">{time}</span>
                <Tag minimal intent={LEVEL_INTENT[entry.level] ?? 'none'} className="log-level-tag">
                  {entry.level}
                </Tag>
                <span className="log-message">{entry.message}</span>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
};
