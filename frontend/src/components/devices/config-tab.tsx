const configEntries = [
  { key: 'reporting_interval', value: '30s' },
  { key: 'max_retries', value: '3' },
  { key: 'protocol', value: 'MQTT v5' },
  { key: 'encryption', value: 'TLS 1.3' },
  { key: 'data_format', value: 'Protobuf' },
  { key: 'keepalive', value: '60s' },
];

export const ConfigTab = () => (
  <div className="config-tab">
    {configEntries.map((entry) => (
      <div key={entry.key} className="config-row">
        <span className="config-key mono-data">{entry.key}</span>
        <span className="config-value mono-data">{entry.value}</span>
      </div>
    ))}
  </div>
);
