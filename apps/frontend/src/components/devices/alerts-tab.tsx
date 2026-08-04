import { useMemo } from 'react';
import { HTMLTable, Tag, Icon, Button, Spinner, Callout, H4 } from '@blueprintjs/core';
import { useAlerts, useAcknowledgeAlert, useResolveAlert } from '../../features/alerts';
import { showSuccessToast, showErrorToast } from '../../utils/toaster';
import type { Alert } from '../../types/alerts';

interface AlertsTabProps {
  deviceId: string;
}

export function AlertsTab({ deviceId }: AlertsTabProps) {
  const alertsQuery = useAlerts({ device_id: deviceId });
  const acknowledgeMutation = useAcknowledgeAlert();
  const resolveMutation = useResolveAlert();

  const alerts = useMemo(() => alertsQuery.data?.data ?? [], [alertsQuery.data?.data]);

  const handleAcknowledge = (alert: Alert) => {
    acknowledgeMutation.mutate(alert.id, {
      onSuccess: () => void showSuccessToast('Alert acknowledged'),
      onError: () => void showErrorToast('Failed to acknowledge alert'),
    });
  };

  const handleResolve = (alert: Alert) => {
    resolveMutation.mutate(alert.id, {
      onSuccess: () => void showSuccessToast('Alert resolved'),
      onError: () => void showErrorToast('Failed to resolve alert'),
    });
  };

  const severityIcon = (severity: string) => {
    switch (severity) {
      case 'critical':
        return 'error';
      case 'warning':
        return 'warning-sign';
      default:
        return 'info-sign';
    }
  };

  const severityClass = (severity: string) => `severity-icon--${severity}`;

  const statusIntent = (status: string) => {
    switch (status) {
      case 'active':
        return 'danger' as const;
      case 'acknowledged':
        return 'warning' as const;
      case 'resolved':
        return 'success' as const;
      default:
        return 'none' as const;
    }
  };

  const formatTime = (iso: string) => {
    try {
      return new Date(iso).toLocaleString();
    } catch {
      return iso;
    }
  };

  if (alertsQuery.isLoading) return <Spinner />;

  if (alertsQuery.error) {
    return (
      <Callout intent="danger" icon="error">
        Failed to load alerts for this device.
      </Callout>
    );
  }

  if (alerts.length === 0) {
    return (
      <div className="empty-state">
        <Icon icon="tick-circle" size={48} />
        <H4>No alerts</H4>
        <p>This device has no alerts</p>
      </div>
    );
  }

  return (
    <HTMLTable interactive className="tab-table">
      <thead>
        <tr>
          <th style={{ width: 40 }}>Sev</th>
          <th>Message</th>
          <th>Status</th>
          <th>Created</th>
          <th style={{ width: 100 }}>Actions</th>
        </tr>
      </thead>
      <tbody>
        {alerts.map((alert) => (
          <tr key={alert.id}>
            <td>
              <Icon
                icon={severityIcon(alert.severity)}
                size={16}
                className={severityClass(alert.severity)}
              />
            </td>
            <td>{alert.message}</td>
            <td>
              <Tag intent={statusIntent(alert.status)} minimal>
                {alert.status}
              </Tag>
            </td>
            <td>
              <span className="mono-data" style={{ fontSize: 12 }}>
                {formatTime(alert.created_at)}
              </span>
            </td>
            <td>
              <div style={{ display: 'flex', gap: 4 }}>
                {alert.status === 'active' && (
                  <Button
                    icon="tick"
                    minimal
                    small
                    title="Acknowledge"
                    onClick={() => handleAcknowledge(alert)}
                  />
                )}
                {(alert.status === 'active' || alert.status === 'acknowledged') && (
                  <Button
                    icon="tick-circle"
                    minimal
                    small
                    intent="success"
                    title="Resolve"
                    onClick={() => handleResolve(alert)}
                  />
                )}
              </div>
            </td>
          </tr>
        ))}
      </tbody>
    </HTMLTable>
  );
}
