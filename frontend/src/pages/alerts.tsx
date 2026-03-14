import { useMemo, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import {
  Card,
  Checkbox,
  Elevation,
  H3,
  HTMLTable,
  Tag,
  Button,
  Icon,
  H4,
  Callout,
  Spinner,
  Tooltip,
} from '@blueprintjs/core';
import {
  useAlerts,
  useAcknowledgeAlert,
  useResolveAlert,
  useBulkAcknowledge,
  useBulkResolve,
} from '../hooks/use-alerts';
import { showSuccessToast, showErrorToast } from '../utils/toaster';
import type { Alert } from '../types/alerts';
import './alerts.css';

export const Alerts = () => {
  const navigate = useNavigate();
  const [filterStatus, setFilterStatus] = useState<string>('active');
  const [filterSeverity, setFilterSeverity] = useState<string>('all');
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());

  const queryParams: Record<string, unknown> = {};
  if (filterStatus !== 'all') queryParams.status = filterStatus;
  if (filterSeverity !== 'all') queryParams.severity = filterSeverity;

  const alertsQuery = useAlerts(Object.keys(queryParams).length > 0 ? queryParams : undefined);
  const acknowledgeMutation = useAcknowledgeAlert();
  const resolveMutation = useResolveAlert();
  const bulkAckMutation = useBulkAcknowledge();
  const bulkResolveMutation = useBulkResolve();

  const alerts = useMemo(() => alertsQuery.data?.data ?? [], [alertsQuery.data?.data]);
  const isLoading = alertsQuery.isLoading;
  const error = alertsQuery.error;

  const toggleSelected = (id: string) => {
    setSelectedIds((prev) => {
      const next = new Set(prev);
      if (next.has(id)) {
        next.delete(id);
      } else {
        next.add(id);
      }
      return next;
    });
  };

  const toggleAll = () => {
    if (alerts.every((a) => selectedIds.has(a.id))) {
      setSelectedIds(new Set());
    } else {
      setSelectedIds(new Set(alerts.map((a) => a.id)));
    }
  };

  const hasSelection = selectedIds.size > 0;

  const handleAcknowledge = (alert: Alert, e: React.MouseEvent) => {
    e.stopPropagation();
    acknowledgeMutation.mutate(alert.id, {
      onSuccess: () => void showSuccessToast('Alert acknowledged'),
      onError: () => void showErrorToast('Failed to acknowledge alert'),
    });
  };

  const handleResolve = (alert: Alert, e: React.MouseEvent) => {
    e.stopPropagation();
    resolveMutation.mutate(alert.id, {
      onSuccess: () => void showSuccessToast('Alert resolved'),
      onError: () => void showErrorToast('Failed to resolve alert'),
    });
  };

  const handleBulkAcknowledge = () => {
    const ids = Array.from(selectedIds);
    bulkAckMutation.mutate(ids, {
      onSuccess: () => {
        void showSuccessToast(`${ids.length} alert(s) acknowledged`);
        setSelectedIds(new Set());
      },
      onError: () => void showErrorToast('Failed to acknowledge alerts'),
    });
  };

  const handleBulkResolve = () => {
    const ids = Array.from(selectedIds);
    bulkResolveMutation.mutate(ids, {
      onSuccess: () => {
        void showSuccessToast(`${ids.length} alert(s) resolved`);
        setSelectedIds(new Set());
      },
      onError: () => void showErrorToast('Failed to resolve alerts'),
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
      const d = new Date(iso);
      return d.toLocaleString();
    } catch {
      return iso;
    }
  };

  if (error) {
    return (
      <div className="alerts-page">
        <Callout intent="danger" icon="error">
          Failed to load alerts. Is the backend running?
        </Callout>
      </div>
    );
  }

  if (isLoading) {
    return (
      <div className="alerts-page">
        <Spinner />
      </div>
    );
  }

  return (
    <div className="alerts-page">
      {/* Header */}
      <div className="page-header">
        <div>
          <H3>Alerts</H3>
          <p className="page-description">{alerts.length} alerts</p>
        </div>
      </div>

      {/* Filters */}
      <Card elevation={Elevation.ONE} className="alerts-controls">
        <div className="controls-row">
          <div className="filter-section">
            {(['all', 'active', 'acknowledged', 'resolved'] as const).map((status) => (
              <button
                key={status}
                className={`filter-pill ${filterStatus === status ? 'active' : ''}`}
                onClick={() => {
                  setFilterStatus(status);
                  setSelectedIds(new Set());
                }}
              >
                <span className="pill-label">
                  {status.charAt(0).toUpperCase() + status.slice(1)}
                </span>
              </button>
            ))}
          </div>
          <div className="filter-section">
            {(['all', 'info', 'warning', 'critical'] as const).map((severity) => (
              <button
                key={severity}
                className={`filter-pill ${filterSeverity === severity ? 'active' : ''}`}
                onClick={() => {
                  setFilterSeverity(severity);
                  setSelectedIds(new Set());
                }}
              >
                {severity !== 'all' && (
                  <Icon
                    icon={severityIcon(severity)}
                    size={12}
                    className={`severity-icon--${severity}`}
                  />
                )}
                <span className="pill-label">
                  {severity.charAt(0).toUpperCase() + severity.slice(1)}
                </span>
              </button>
            ))}
          </div>
        </div>
      </Card>

      {/* Bulk action bar */}
      {hasSelection && (
        <div className="alerts-bulk-bar">
          <div className="alerts-bulk-bar-left">
            <span>{selectedIds.size} alert(s) selected</span>
            <Button minimal small onClick={() => setSelectedIds(new Set())}>
              Clear
            </Button>
          </div>
          <div className="alerts-bulk-bar-right">
            <Button
              icon="tick"
              small
              intent="warning"
              onClick={handleBulkAcknowledge}
              loading={bulkAckMutation.isPending}
            >
              Acknowledge
            </Button>
            <Button
              icon="tick-circle"
              small
              intent="success"
              onClick={handleBulkResolve}
              loading={bulkResolveMutation.isPending}
            >
              Resolve
            </Button>
          </div>
        </div>
      )}

      {/* Alerts Table */}
      <Card elevation={Elevation.ONE} className="alerts-card">
        {alerts.length === 0 ? (
          <div className="empty-state">
            <Icon icon="tick-circle" size={48} />
            <H4>No alerts</H4>
            <p>No alerts matching your current filters</p>
          </div>
        ) : (
          <HTMLTable interactive className="alerts-table">
            <thead>
              <tr>
                <th style={{ width: 40 }}>
                  <Checkbox
                    checked={alerts.length > 0 && alerts.every((a) => selectedIds.has(a.id))}
                    indeterminate={
                      alerts.some((a) => selectedIds.has(a.id)) &&
                      !alerts.every((a) => selectedIds.has(a.id))
                    }
                    onChange={toggleAll}
                    style={{ marginBottom: 0 }}
                  />
                </th>
                <th style={{ width: 40 }}>Severity</th>
                <th>Message</th>
                <th>Device</th>
                <th>Status</th>
                <th>Created</th>
                <th style={{ width: 100 }}>Actions</th>
              </tr>
            </thead>
            <tbody>
              {alerts.map((alert) => (
                <tr
                  key={alert.id}
                  className={`alert-row alert-row--${alert.severity} ${selectedIds.has(alert.id) ? 'alert-row--selected' : ''}`}
                >
                  <td onClick={(e) => e.stopPropagation()}>
                    <Checkbox
                      checked={selectedIds.has(alert.id)}
                      onChange={() => toggleSelected(alert.id)}
                      style={{ marginBottom: 0 }}
                    />
                  </td>
                  <td>
                    <Icon
                      icon={severityIcon(alert.severity)}
                      size={16}
                      className={`severity-icon--${alert.severity}`}
                    />
                  </td>
                  <td>
                    <span className="alert-message-cell">{alert.message}</span>
                  </td>
                  <td>
                    <span
                      className="alert-device-link mono-data"
                      onClick={(e) => {
                        e.stopPropagation();
                        navigate(`/devices/${alert.device_id}`);
                      }}
                      title={alert.device_id}
                    >
                      {alert.device_id.length > 12
                        ? `${alert.device_id.slice(0, 12)}...`
                        : alert.device_id}
                    </span>
                  </td>
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
                    <div className="alert-row-actions">
                      {alert.status === 'active' && (
                        <Tooltip content="Acknowledge" minimal hoverOpenDelay={150}>
                          <Button
                            icon="tick"
                            minimal
                            small
                            onClick={(e) => handleAcknowledge(alert, e)}
                          />
                        </Tooltip>
                      )}
                      {(alert.status === 'active' || alert.status === 'acknowledged') && (
                        <Tooltip content="Resolve" minimal hoverOpenDelay={150}>
                          <Button
                            icon="tick-circle"
                            minimal
                            small
                            intent="success"
                            onClick={(e) => handleResolve(alert, e)}
                          />
                        </Tooltip>
                      )}
                    </div>
                  </td>
                </tr>
              ))}
            </tbody>
          </HTMLTable>
        )}
      </Card>
    </div>
  );
};
