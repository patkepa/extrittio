import { useMemo, useState } from 'react';
import { Link } from 'react-router-dom';
import {
  Card,
  Checkbox,
  Elevation,
  H3,
  HTMLTable,
  Tag,
  Button,
  Icon,
  Callout,
  Spinner,
  Tooltip,
} from '@blueprintjs/core';
import {
  useAlerts,
  useAcknowledgeAlert,
  useResolveAlert,
  useReactivateAlert,
  useBulkAcknowledge,
  useBulkResolve,
  useBulkReactivate,
} from '../queries/use-alerts';
import { EmptyState, FilterPill } from '@patkepa/kantzen-ui';
import { showSuccessToast, showErrorToast } from '../../../utils/toaster';
import type { Alert } from '../../../types/alerts';
import { hasPermission } from '../../../auth/permissions';
import { useAuthStore } from '../../../stores/auth-store';
import './alerts.css';

export const Alerts = () => {
  const permissions = useAuthStore((s) => s.user?.permissions);
  const canManageAlerts = hasPermission(permissions, 'alerts.manage');
  const [filterStatus, setFilterStatus] = useState<string>('active');
  const [filterSeverity, setFilterSeverity] = useState<string>('all');
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());
  const [page, setPage] = useState(0);
  const pageSize = 50;

  const queryParams: Record<string, unknown> = { limit: pageSize, offset: page * pageSize };
  if (filterStatus !== 'all') queryParams.status = filterStatus;
  if (filterSeverity !== 'all') queryParams.severity = filterSeverity;

  const alertsQuery = useAlerts(Object.keys(queryParams).length > 0 ? queryParams : undefined);
  const acknowledgeMutation = useAcknowledgeAlert();
  const resolveMutation = useResolveAlert();
  const reactivateMutation = useReactivateAlert();
  const bulkAckMutation = useBulkAcknowledge();
  const bulkResolveMutation = useBulkResolve();
  const bulkReactivateMutation = useBulkReactivate();

  const alerts = useMemo(() => alertsQuery.data?.data ?? [], [alertsQuery.data?.data]);
  const total = alertsQuery.data?.total ?? 0;
  const totalPages = Math.ceil(total / pageSize);
  const isLoading = alertsQuery.isLoading;
  const error = alertsQuery.error;

  const selectedOnPage = useMemo(() => {
    const visibleIds = new Set(alerts.map((alert) => alert.id));
    return new Set([...selectedIds].filter((id) => visibleIds.has(id)));
  }, [alerts, selectedIds]);
  const selectedAlerts = alerts.filter((alert) => selectedOnPage.has(alert.id));
  const canAcknowledgeSelection = selectedAlerts.every((alert) => alert.status === 'active');
  const canResolveSelection = selectedAlerts.every(
    (alert) => alert.status === 'active' || alert.status === 'acknowledged',
  );
  const canReactivateSelection = selectedAlerts.every(
    (alert) => alert.status === 'acknowledged' || alert.status === 'resolved',
  );

  const toggleSelected = (id: string) => {
    setSelectedIds((prev) => {
      const visibleIds = new Set(alerts.map((alert) => alert.id));
      const next = new Set([...prev].filter((selectedId) => visibleIds.has(selectedId)));
      if (next.has(id)) {
        next.delete(id);
      } else {
        next.add(id);
      }
      return next;
    });
  };

  const toggleAll = () => {
    if (alerts.every((a) => selectedOnPage.has(a.id))) {
      setSelectedIds(new Set());
    } else {
      setSelectedIds(new Set(alerts.map((a) => a.id)));
    }
  };

  const hasSelection = canManageAlerts && selectedOnPage.size > 0;
  const changePage = (nextPage: number) => {
    setSelectedIds(new Set());
    setPage(nextPage);
  };

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

  const handleReactivate = (alert: Alert, e: React.MouseEvent) => {
    e.stopPropagation();
    reactivateMutation.mutate(alert.id, {
      onSuccess: () => void showSuccessToast('Alert reactivated'),
      onError: () => void showErrorToast('Failed to reactivate alert'),
    });
  };

  const handleBulkReactivate = () => {
    const ids = Array.from(selectedOnPage);
    bulkReactivateMutation.mutate(ids, {
      onSuccess: () => {
        void showSuccessToast(`${ids.length} alert(s) reactivated`);
        setSelectedIds(new Set());
      },
      onError: () => void showErrorToast('Failed to reactivate alerts'),
    });
  };

  const handleBulkAcknowledge = () => {
    const ids = Array.from(selectedOnPage);
    bulkAckMutation.mutate(ids, {
      onSuccess: () => {
        void showSuccessToast(`${ids.length} alert(s) acknowledged`);
        setSelectedIds(new Set());
      },
      onError: () => void showErrorToast('Failed to acknowledge alerts'),
    });
  };

  const handleBulkResolve = () => {
    const ids = Array.from(selectedOnPage);
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
          <p className="page-description">{total} alerts</p>
        </div>
      </div>

      {/* Filters */}
      <Card elevation={Elevation.ONE} className="alerts-controls">
        <div className="controls-row">
          <div className="filter-section">
            {(['all', 'active', 'acknowledged', 'resolved'] as const).map((status) => (
              <FilterPill
                key={status}
                value={status}
                label={status.charAt(0).toUpperCase() + status.slice(1)}
                active={filterStatus === status}
                onSelect={(nextStatus) => {
                  setFilterStatus(nextStatus);
                  setSelectedIds(new Set());
                  setPage(0);
                }}
              />
            ))}
          </div>
          <div className="filter-section">
            {(['all', 'info', 'warning', 'critical'] as const).map((severity) => (
              <FilterPill
                key={severity}
                value={severity}
                label={severity.charAt(0).toUpperCase() + severity.slice(1)}
                active={filterSeverity === severity}
                icon={severity !== 'all' ? severityIcon(severity) : undefined}
                iconClassName={severity !== 'all' ? `severity-icon--${severity}` : undefined}
                onSelect={(nextSeverity) => {
                  setFilterSeverity(nextSeverity);
                  setSelectedIds(new Set());
                  setPage(0);
                }}
              />
            ))}
          </div>
        </div>
      </Card>

      {/* Bulk action bar */}
      {hasSelection && (
        <div className="alerts-bulk-bar">
          <div className="alerts-bulk-bar-left">
            <span>{selectedOnPage.size} alert(s) selected on this page</span>
            <Button minimal small onClick={() => setSelectedIds(new Set())}>
              Clear
            </Button>
          </div>
          <div className="alerts-bulk-bar-right">
            {canAcknowledgeSelection && (
              <Button
                icon="tick"
                small
                intent="warning"
                onClick={handleBulkAcknowledge}
                loading={bulkAckMutation.isPending}
              >
                Acknowledge
              </Button>
            )}
            {canResolveSelection && (
              <Button
                icon="tick-circle"
                small
                intent="success"
                onClick={handleBulkResolve}
                loading={bulkResolveMutation.isPending}
              >
                Resolve
              </Button>
            )}
            {canReactivateSelection && (
              <Button
                icon="undo"
                small
                intent="primary"
                onClick={handleBulkReactivate}
                loading={bulkReactivateMutation.isPending}
              >
                Reactivate
              </Button>
            )}
          </div>
        </div>
      )}

      {/* Alerts Table */}
      <Card elevation={Elevation.ONE} className="alerts-card">
        {alerts.length === 0 ? (
          <EmptyState
            icon="tick-circle"
            title="No alerts"
            description="No alerts matching your current filters"
          />
        ) : (
          <HTMLTable interactive className="alerts-table">
            <thead>
              <tr>
                {canManageAlerts && (
                  <th style={{ width: 40 }}>
                    <Checkbox
                      aria-label="Select all alerts on this page"
                      checked={alerts.length > 0 && alerts.every((a) => selectedOnPage.has(a.id))}
                      indeterminate={
                        alerts.some((a) => selectedOnPage.has(a.id)) &&
                        !alerts.every((a) => selectedOnPage.has(a.id))
                      }
                      onChange={toggleAll}
                      style={{ marginBottom: 0 }}
                    />
                  </th>
                )}
                <th style={{ width: 40 }}>Severity</th>
                <th>Message</th>
                <th>Device</th>
                <th>Status</th>
                <th>Created</th>
                {canManageAlerts && <th style={{ width: 100 }}>Actions</th>}
              </tr>
            </thead>
            <tbody>
              {alerts.map((alert) => (
                <tr
                  key={alert.id}
                  className={`alert-row alert-row--${alert.severity} ${selectedOnPage.has(alert.id) ? 'alert-row--selected' : ''}`}
                >
                  {canManageAlerts && (
                    <td onClick={(e) => e.stopPropagation()}>
                      <Checkbox
                        aria-label={`Select alert: ${alert.message}`}
                        checked={selectedOnPage.has(alert.id)}
                        onChange={() => toggleSelected(alert.id)}
                        style={{ marginBottom: 0 }}
                      />
                    </td>
                  )}
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
                    <Link
                      className="alert-device-link mono-data"
                      to={`/devices/${encodeURIComponent(alert.device_id)}`}
                      title={alert.device_id}
                    >
                      {alert.device_id.length > 12
                        ? `${alert.device_id.slice(0, 12)}...`
                        : alert.device_id}
                    </Link>
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
                  {canManageAlerts && (
                    <td>
                      <div className="alert-row-actions">
                        {alert.status === 'active' && (
                          <Tooltip content="Acknowledge" minimal hoverOpenDelay={150}>
                            <Button
                              icon="tick"
                              aria-label="Acknowledge alert"
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
                              aria-label="Resolve alert"
                              minimal
                              small
                              intent="success"
                              onClick={(e) => handleResolve(alert, e)}
                            />
                          </Tooltip>
                        )}
                        {(alert.status === 'acknowledged' || alert.status === 'resolved') && (
                          <Tooltip content="Reactivate" minimal hoverOpenDelay={150}>
                            <Button
                              icon="undo"
                              aria-label="Reactivate alert"
                              minimal
                              small
                              intent="primary"
                              onClick={(e) => handleReactivate(alert, e)}
                            />
                          </Tooltip>
                        )}
                      </div>
                    </td>
                  )}
                </tr>
              ))}
            </tbody>
          </HTMLTable>
        )}
      </Card>

      {/* Pagination */}
      {totalPages > 1 && (
        <div className="pagination-controls">
          <Button
            icon="chevron-left"
            minimal
            disabled={page === 0}
            onClick={() => changePage(Math.max(0, page - 1))}
          />
          <span className="pagination-info mono-data">
            Page {page + 1} of {totalPages}
          </span>
          <Button
            icon="chevron-right"
            minimal
            disabled={page >= totalPages - 1}
            onClick={() => changePage(Math.min(totalPages - 1, page + 1))}
          />
        </div>
      )}
    </div>
  );
};
