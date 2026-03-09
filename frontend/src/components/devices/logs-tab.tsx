const logEntries = [
  { time: '14:32:01', level: 'INFO', message: 'Device heartbeat received' },
  { time: '14:30:45', level: 'INFO', message: 'Telemetry data uploaded (128 bytes)' },
  { time: '14:28:12', level: 'WARN', message: 'Signal strength below threshold' },
  { time: '14:25:00', level: 'INFO', message: 'Configuration sync completed' },
  { time: '14:20:33', level: 'ERROR', message: 'Connection timeout — retrying' },
  { time: '14:18:15', level: 'INFO', message: 'Firmware check: up to date' },
];

export const LogsTab = () => (
  <div className="logs-tab">
    {logEntries.map((entry, i) => (
      <div key={i} className="log-entry">
        <span className="log-time mono-data">{entry.time}</span>
        <span className={`log-level log-level--${entry.level.toLowerCase()} mono-data`}>
          {entry.level}
        </span>
        <span className="log-message">{entry.message}</span>
      </div>
    ))}
  </div>
);
