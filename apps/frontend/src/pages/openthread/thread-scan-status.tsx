import { Tag } from '@blueprintjs/core';

export function ThreadScanStatus({
  connected,
  scanning,
  scannedAt,
  error,
}: {
  connected: boolean;
  scanning: boolean;
  scannedAt?: string | null;
  error?: string | null;
}) {
  if (!connected) {
    return (
      <Tag icon="offline" intent="warning" minimal round>
        Scanner unavailable
      </Tag>
    );
  }
  if (scanning) {
    return (
      <Tag icon="search" intent="primary" round>
        Scanning now
      </Tag>
    );
  }
  if (error) {
    return (
      <Tag icon="warning-sign" intent="warning" minimal round title={error}>
        Diagnostics incomplete
      </Tag>
    );
  }
  if (scannedAt) {
    return (
      <Tag icon="pulse" intent="success" minimal round>
        Monitoring automatically
      </Tag>
    );
  }
  return (
    <Tag icon="time" minimal round>
      Preparing scanner
    </Tag>
  );
}
