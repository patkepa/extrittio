import { useMemo, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import {
  Button,
  Callout,
  Card,
  Elevation,
  H3,
  HTMLTable,
  Icon,
  ProgressBar,
  Spinner,
  Tag,
} from '@blueprintjs/core';
import { EmptyState, FilterPill } from '@extrittio/ui';
import { useAllOtaDeployments } from '../hooks/use-firmware-updates';
import type { GlobalOtaDeployment } from '../types/api';
import './updates.css';

type UpdateFilter = 'in_progress' | 'completed' | 'all';

const pageSize = 50;
const emptyDeployments: GlobalOtaDeployment[] = [];

function statusIntent(status: string) {
  switch (status) {
    case 'success':
      return 'success' as const;
    case 'failed':
      return 'danger' as const;
    case 'pending':
      return 'warning' as const;
    default:
      return 'primary' as const;
  }
}

function progressForStatus(status: string): number {
  switch (status) {
    case 'pending':
      return 0.18;
    case 'downloading':
      return 0.4;
    case 'verifying':
      return 0.65;
    case 'installing':
      return 0.82;
    case 'success':
      return 1;
    case 'failed':
      return 1;
    default:
      return 0.28;
  }
}

function formatStatus(status: string): string {
  return status.replace(/_/g, ' ').replace(/\b\w/g, (char) => char.toUpperCase());
}

function formatTime(value: string | null): string {
  if (!value) return '-';

  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;

  return date.toLocaleString();
}

function formatDuration(startValue: string, endValue: string | null): string {
  const start = new Date(startValue).getTime();
  const end = endValue ? new Date(endValue).getTime() : Date.now();

  if (Number.isNaN(start) || Number.isNaN(end) || end < start) return '-';

  const totalSeconds = Math.round((end - start) / 1000);
  if (totalSeconds < 60) return `${totalSeconds}s`;

  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  if (minutes < 60) return seconds > 0 ? `${minutes}m ${seconds}s` : `${minutes}m`;

  const hours = Math.floor(minutes / 60);
  const remainingMinutes = minutes % 60;
  return remainingMinutes > 0 ? `${hours}h ${remainingMinutes}m` : `${hours}h`;
}

function activeFilterLabel(filter: UpdateFilter): string {
  switch (filter) {
    case 'in_progress':
      return 'in progress';
    case 'completed':
      return 'completed';
    default:
      return 'total';
  }
}

export const Updates = () => {
  const navigate = useNavigate();
  const [filter, setFilter] = useState<UpdateFilter>('in_progress');
  const [page, setPage] = useState(0);

  const params = useMemo(
    () => ({
      status: filter,
      limit: pageSize,
      offset: page * pageSize,
    }),
    [filter, page],
  );

  const deploymentsQuery = useAllOtaDeployments(params);
  const deployments = deploymentsQuery.data?.data ?? emptyDeployments;
  const total = deploymentsQuery.data?.total ?? 0;
  const totalPages = Math.ceil(total / pageSize);

  const counts = useMemo(
    () =>
      deployments.reduce(
        (acc, deployment) => {
          if (deployment.status === 'success') acc.success += 1;
          else if (deployment.status === 'failed') acc.failed += 1;
          else acc.inProgress += 1;
          return acc;
        },
        { inProgress: 0, success: 0, failed: 0 },
      ),
    [deployments],
  );

  const changeFilter = (next: UpdateFilter) => {
    setFilter(next);
    setPage(0);
  };

  if (deploymentsQuery.isError) {
    return (
      <div className="updates-page">
        <Callout intent="danger" icon="error">
          Failed to load update deployments. Is the backend running?
        </Callout>
      </div>
    );
  }

  return (
    <div className="updates-page">
      <div className="page-header">
        <div>
          <H3>Updates</H3>
          <p className="page-description">
            {total} {activeFilterLabel(filter)}
          </p>
        </div>
        <Button icon="upload" onClick={() => navigate('/settings/firmware')}>
          Firmware
        </Button>
      </div>

      <Card elevation={Elevation.ONE} className="updates-controls">
        <div className="filter-section">
          {(['in_progress', 'completed', 'all'] as const).map((status) => (
            <FilterPill
              key={status}
              value={status}
              label={formatStatus(status)}
              active={filter === status}
              onSelect={changeFilter}
            />
          ))}
        </div>
        <div className="updates-page-stats">
          <span>Page</span>
          <span>
            <Icon icon="time" size={12} /> {counts.inProgress} active
          </span>
          <span>
            <Icon icon="tick-circle" size={12} /> {counts.success} success
          </span>
          <span>
            <Icon icon="error" size={12} /> {counts.failed} failed
          </span>
        </div>
      </Card>

      {deploymentsQuery.isLoading ? (
        <div className="updates-loading">
          <Spinner />
        </div>
      ) : deployments.length === 0 ? (
        <Card elevation={Elevation.ONE} className="updates-empty-card">
          <EmptyState icon="updated" title="No updates" className="updates-empty-state" />
        </Card>
      ) : (
        <Card elevation={Elevation.ONE} className="updates-card">
          <HTMLTable interactive className="updates-table">
            <thead>
              <tr>
                <th>Device</th>
                <th>Target</th>
                <th>Status</th>
                <th>Progress</th>
                <th>Started</th>
                <th>Duration</th>
                <th>Fleet</th>
              </tr>
            </thead>
            <tbody>
              {deployments.map((deployment) => (
                <UpdateRow
                  key={deployment.id}
                  deployment={deployment}
                  onDeviceClick={() => navigate(`/devices/${deployment.device_id}?tab=ota`)}
                />
              ))}
            </tbody>
          </HTMLTable>
        </Card>
      )}

      {totalPages > 1 && (
        <div className="updates-pagination">
          <Button
            icon="chevron-left"
            disabled={page === 0}
            onClick={() => setPage((current) => Math.max(0, current - 1))}
          />
          <span>
            Page {page + 1} of {totalPages}
          </span>
          <Button
            icon="chevron-right"
            disabled={page >= totalPages - 1}
            onClick={() => setPage((current) => Math.min(totalPages - 1, current + 1))}
          />
        </div>
      )}
    </div>
  );
};

interface UpdateRowProps {
  deployment: GlobalOtaDeployment;
  onDeviceClick: () => void;
}

function UpdateRow({ deployment, onDeviceClick }: UpdateRowProps) {
  const intent = statusIntent(deployment.status);
  const progress = progressForStatus(deployment.status);
  const isTerminal = deployment.status === 'success' || deployment.status === 'failed';

  return (
    <tr className={`updates-row updates-row--${deployment.status}`}>
      <td>
        <button className="updates-device-link" onClick={onDeviceClick}>
          {deployment.device_name}
        </button>
        <div className="updates-device-meta mono-data">{deployment.device_id}</div>
      </td>
      <td>
        <Tag minimal intent="primary" className="mono-data">
          v{deployment.firmware_version}
        </Tag>
        <div className="updates-device-meta">{deployment.device_type_name}</div>
      </td>
      <td>
        <Tag minimal intent={intent}>
          {formatStatus(deployment.status)}
        </Tag>
        {deployment.error_message && (
          <div className="updates-error-text">{deployment.error_message}</div>
        )}
      </td>
      <td className="updates-progress-cell">
        <ProgressBar value={progress} intent={intent} animate={!isTerminal} />
        <span className="updates-progress-label">{Math.round(progress * 100)}%</span>
      </td>
      <td className="updates-muted-cell">{formatTime(deployment.initiated_at)}</td>
      <td className="updates-muted-cell">
        {formatDuration(deployment.initiated_at, deployment.completed_at)}
      </td>
      <td>{deployment.fleet_name ?? '-'}</td>
    </tr>
  );
}
