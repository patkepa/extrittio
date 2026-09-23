import { useEffect, useMemo, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import {
  Button,
  Callout,
  Card,
  Classes,
  Drawer,
  Elevation,
  H3,
  HTMLSelect,
  HTMLTable,
  Icon,
  InputGroup,
  Spinner,
  Switch,
  Tag,
  Tooltip,
} from '@blueprintjs/core';
import { EmptyState, FilterPill } from '@patkepa/kantzen-ui';
import type { ActivityEvent, ActivityEventsParams } from '../../../types/api';
import { useActivityEvents } from '../queries/use-activity-events';
import './activity-logs.css';

type SourceFilter = 'all' | 'device' | 'audit' | 'alert' | 'command' | 'deployment';
type SeverityFilter = 'all' | 'debug' | 'info' | 'warning' | 'error';
type RangeFilter = '1h' | '24h' | '7d' | '30d' | 'all';

const PAGE_SIZE = 100;
const SOURCE_FILTERS: readonly SourceFilter[] = [
  'all',
  'device',
  'audit',
  'alert',
  'command',
  'deployment',
];
const SEVERITY_FILTERS: readonly SeverityFilter[] = ['all', 'debug', 'info', 'warning', 'error'];

const SEVERITY_INTENT = {
  debug: 'none',
  info: 'primary',
  warning: 'warning',
  error: 'danger',
} as const;

const SOURCE_ICON = {
  device: 'mobile-video',
  audit: 'shield',
  alert: 'warning-sign',
  command: 'console',
  deployment: 'updated',
} as const;

const RANGE_MILLISECONDS: Record<Exclude<RangeFilter, 'all'>, number> = {
  '1h': 60 * 60 * 1_000,
  '24h': 24 * 60 * 60 * 1_000,
  '7d': 7 * 24 * 60 * 60 * 1_000,
  '30d': 30 * 24 * 60 * 60 * 1_000,
};

function useDebouncedValue<T>(value: T, delay: number): T {
  const [debouncedValue, setDebouncedValue] = useState(value);

  useEffect(() => {
    const timeout = window.setTimeout(() => setDebouncedValue(value), delay);
    return () => window.clearTimeout(timeout);
  }, [delay, value]);

  return debouncedValue;
}

function titleCase(value: string): string {
  return value.replace(/[._-]+/g, ' ').replace(/\b\w/g, (character) => character.toUpperCase());
}

function formatTimestamp(value: string): string {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return date.toLocaleString(undefined, {
    year: 'numeric',
    month: 'short',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit',
  });
}

function formatEventTarget(event: ActivityEvent): string {
  return event.resource_id ? `${event.resource_type} / ${event.resource_id}` : event.resource_type;
}

function EventDetails({ event, onClose }: { event: ActivityEvent; onClose: () => void }) {
  const navigate = useNavigate();
  const deviceId =
    event.actor_type === 'device'
      ? event.actor_id
      : event.resource_type === 'device'
        ? event.resource_id
        : null;

  return (
    <Drawer
      className="activity-details-drawer"
      isOpen
      onClose={onClose}
      position="right"
      size="min(560px, 94vw)"
      title="Event details"
      icon="document"
    >
      <div className={Classes.DRAWER_BODY}>
        <div className={Classes.DIALOG_BODY}>
          <div className="activity-details-heading">
            <div className="activity-details-tags">
              <Tag
                minimal
                intent={SEVERITY_INTENT[event.severity as keyof typeof SEVERITY_INTENT] ?? 'none'}
              >
                {event.severity.toUpperCase()}
              </Tag>
              <Tag minimal icon={SOURCE_ICON[event.source as keyof typeof SOURCE_ICON] ?? 'feed'}>
                {titleCase(event.source)}
              </Tag>
            </div>
            <H3>{event.message}</H3>
            <span className="activity-details-time mono-data">
              {formatTimestamp(event.occurred_at)}
            </span>
          </div>

          <dl className="activity-details-grid">
            <div>
              <dt>Event type</dt>
              <dd className="mono-data">{event.event_type}</dd>
            </div>
            <div>
              <dt>Category</dt>
              <dd>{titleCase(event.category)}</dd>
            </div>
            <div>
              <dt>Actor</dt>
              <dd>
                {event.actor_type}
                {event.actor_id ? ` / ${event.actor_id}` : ''}
              </dd>
            </div>
            <div>
              <dt>Resource</dt>
              <dd>{formatEventTarget(event)}</dd>
            </div>
            {event.request_id ? (
              <div className="activity-details-wide">
                <dt>Request ID</dt>
                <dd className="mono-data">{event.request_id}</dd>
              </div>
            ) : null}
            <div className="activity-details-wide">
              <dt>Event ID</dt>
              <dd className="mono-data">{event.id}</dd>
            </div>
          </dl>

          {deviceId ? (
            <Button
              icon="mobile-video"
              minimal
              className="activity-device-action"
              onClick={() => navigate(`/devices/${deviceId}`)}
            >
              Open device
            </Button>
          ) : null}

          <div className="activity-metadata-header">
            <span>Metadata</span>
            <Button
              icon="duplicate"
              minimal
              small
              onClick={() =>
                void navigator.clipboard.writeText(JSON.stringify(event.metadata, null, 2))
              }
            >
              Copy JSON
            </Button>
          </div>
          <pre className="activity-metadata mono-data">
            {JSON.stringify(event.metadata, null, 2)}
          </pre>
        </div>
      </div>
    </Drawer>
  );
}

export const ActivityLogs = () => {
  const [source, setSource] = useState<SourceFilter>('all');
  const [severity, setSeverity] = useState<SeverityFilter>('all');
  const [range, setRange] = useState<RangeFilter>('all');
  const [rangeAnchor, setRangeAnchor] = useState(() => Date.now());
  const [search, setSearch] = useState('');
  const [page, setPage] = useState(0);
  const [live, setLive] = useState(true);
  const [selectedEvent, setSelectedEvent] = useState<ActivityEvent | null>(null);
  const debouncedSearch = useDebouncedValue(search.trim(), 350);

  useEffect(() => {
    if (!live || range === 'all') return;
    const timer = window.setInterval(() => setRangeAnchor(Date.now()), 10_000);
    return () => window.clearInterval(timer);
  }, [live, range]);

  const since =
    range === 'all' ? undefined : new Date(rangeAnchor - RANGE_MILLISECONDS[range]).toISOString();

  const params = useMemo<ActivityEventsParams>(
    () => ({
      limit: PAGE_SIZE,
      offset: page * PAGE_SIZE,
      ...(source === 'all' ? {} : { source }),
      ...(severity === 'all' ? {} : { severity }),
      ...(debouncedSearch ? { search: debouncedSearch } : {}),
      ...(since ? { since } : {}),
    }),
    [debouncedSearch, page, severity, since, source],
  );

  const eventsQuery = useActivityEvents(params, {
    refetchInterval: live && range === 'all' ? 10_000 : false,
  });
  const events = eventsQuery.data?.data ?? [];
  const total = eventsQuery.data?.total ?? 0;
  const totalPages = Math.ceil(total / PAGE_SIZE);
  const hasFilters = source !== 'all' || severity !== 'all' || range !== 'all' || search !== '';

  const resetPage = () => setPage(0);
  const resetFilters = () => {
    setSource('all');
    setSeverity('all');
    setRange('all');
    setRangeAnchor(Date.now());
    setSearch('');
    setPage(0);
  };

  return (
    <div className="activity-page">
      <div className="page-header">
        <div>
          <H3>Logs</H3>
          <p className="page-description">
            Tenant-wide operational events, device logs, and administrative audit activity
          </p>
        </div>
        <div className="activity-header-actions">
          <Switch
            checked={live}
            label="Live updates"
            onChange={(event) => {
              setLive(event.currentTarget.checked);
              setRangeAnchor(Date.now());
            }}
          />
          <Tooltip content="Refresh event stream" minimal>
            <Button
              icon="refresh"
              loading={eventsQuery.isFetching}
              onClick={() => {
                if (range === 'all') void eventsQuery.refetch();
                else setRangeAnchor(Date.now());
              }}
            >
              Refresh
            </Button>
          </Tooltip>
        </div>
      </div>

      <Card elevation={Elevation.ONE} className="activity-controls">
        <div className="activity-search-row">
          <InputGroup
            leftIcon="search"
            placeholder="Search messages, event types, actors, resources, or request IDs"
            value={search}
            onChange={(event) => {
              setSearch(event.target.value);
              resetPage();
            }}
            rightElement={
              search ? (
                <Button
                  icon="cross"
                  minimal
                  onClick={() => setSearch('')}
                  aria-label="Clear search"
                />
              ) : undefined
            }
          />
          <HTMLSelect
            aria-label="Time range"
            value={range}
            onChange={(event) => {
              const nextRange = event.target.value as RangeFilter;
              setRange(nextRange);
              setRangeAnchor(Date.now());
              resetPage();
            }}
            options={[
              { value: '1h', label: 'Last hour' },
              { value: '24h', label: 'Last 24 hours' },
              { value: '7d', label: 'Last 7 days' },
              { value: '30d', label: 'Last 30 days' },
              { value: 'all', label: 'All time' },
            ]}
          />
        </div>

        <div className="activity-filter-row">
          <div className="activity-filter-group" aria-label="Source filter">
            <span className="activity-filter-label">Source</span>
            {SOURCE_FILTERS.map((value) => (
              <FilterPill
                key={value}
                value={value}
                label={value === 'all' ? 'All sources' : titleCase(value)}
                active={source === value}
                icon={value === 'all' ? 'feed' : SOURCE_ICON[value]}
                onSelect={(next) => {
                  setSource(next);
                  resetPage();
                }}
              />
            ))}
          </div>
          <div className="activity-filter-group" aria-label="Severity filter">
            <span className="activity-filter-label">Severity</span>
            {SEVERITY_FILTERS.map((value) => (
              <FilterPill
                key={value}
                value={value}
                label={titleCase(value)}
                active={severity === value}
                onSelect={(next) => {
                  setSeverity(next);
                  resetPage();
                }}
              />
            ))}
          </div>
        </div>
      </Card>

      <div className="activity-summary">
        <span>
          <strong>{total.toLocaleString()}</strong> matching events
        </span>
        {eventsQuery.dataUpdatedAt ? (
          <span className="mono-data">
            Updated {new Date(eventsQuery.dataUpdatedAt).toLocaleTimeString()}
          </span>
        ) : null}
      </div>

      {eventsQuery.isError ? (
        <Callout intent="danger" icon="error" className="activity-error">
          Failed to load the event stream. Check the backend connection and your log permissions.
        </Callout>
      ) : (
        <Card elevation={Elevation.ONE} className="activity-table-card">
          {eventsQuery.isLoading ? (
            <div className="activity-loading">
              <Spinner />
              <span>Loading event stream…</span>
            </div>
          ) : events.length === 0 ? (
            <div className="activity-empty">
              <EmptyState
                icon="document-open"
                title={hasFilters ? 'No matching events' : 'No events yet'}
                description={
                  hasFilters
                    ? 'Adjust the filters or expand the selected time range.'
                    : 'Device logs and administrative activity will appear here.'
                }
              />
              {hasFilters ? (
                <Button minimal icon="filter-remove" onClick={resetFilters}>
                  Reset filters
                </Button>
              ) : null}
            </div>
          ) : (
            <div className="activity-table-scroll">
              <HTMLTable interactive className="activity-table">
                <thead>
                  <tr>
                    <th>Timestamp</th>
                    <th>Severity</th>
                    <th>Source</th>
                    <th>Event</th>
                    <th>Message</th>
                    <th>Resource</th>
                    <th aria-label="Open details" />
                  </tr>
                </thead>
                <tbody>
                  {events.map((event) => (
                    <tr
                      key={event.id}
                      className={`activity-row activity-row--${event.severity}`}
                      onClick={() => setSelectedEvent(event)}
                      onKeyDown={(keyboardEvent) => {
                        if (keyboardEvent.key === 'Enter' || keyboardEvent.key === ' ') {
                          keyboardEvent.preventDefault();
                          setSelectedEvent(event);
                        }
                      }}
                      tabIndex={0}
                    >
                      <td className="activity-time mono-data">
                        {formatTimestamp(event.occurred_at)}
                      </td>
                      <td>
                        <Tag
                          minimal
                          intent={
                            SEVERITY_INTENT[event.severity as keyof typeof SEVERITY_INTENT] ??
                            'none'
                          }
                        >
                          {event.severity.toUpperCase()}
                        </Tag>
                      </td>
                      <td>
                        <span className="activity-source">
                          <Icon
                            icon={SOURCE_ICON[event.source as keyof typeof SOURCE_ICON] ?? 'feed'}
                            size={14}
                          />
                          {titleCase(event.source)}
                        </span>
                      </td>
                      <td>
                        <span className="activity-event-type mono-data">{event.event_type}</span>
                      </td>
                      <td>
                        <span className="activity-message" title={event.message}>
                          {event.message}
                        </span>
                      </td>
                      <td>
                        <span className="activity-resource" title={formatEventTarget(event)}>
                          {formatEventTarget(event)}
                        </span>
                      </td>
                      <td>
                        <Icon icon="chevron-right" size={14} className="activity-row-chevron" />
                      </td>
                    </tr>
                  ))}
                </tbody>
              </HTMLTable>
            </div>
          )}
        </Card>
      )}

      {totalPages > 1 ? (
        <div className="activity-pagination">
          <Button
            icon="chevron-left"
            minimal
            disabled={page === 0}
            onClick={() => setPage((current) => Math.max(0, current - 1))}
          />
          <span className="mono-data">
            Page {page + 1} of {totalPages}
          </span>
          <Button
            icon="chevron-right"
            minimal
            disabled={page >= totalPages - 1}
            onClick={() => setPage((current) => Math.min(totalPages - 1, current + 1))}
          />
        </div>
      ) : null}

      {selectedEvent ? (
        <EventDetails event={selectedEvent} onClose={() => setSelectedEvent(null)} />
      ) : null}
    </div>
  );
};
