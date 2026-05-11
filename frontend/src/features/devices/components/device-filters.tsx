import { Card, Elevation, Button, InputGroup } from '@blueprintjs/core';
import { BulkActionBar } from '../../../components/devices/bulk-action-bar';
import { StatusLed } from '../../../lib/ui';
import type { BulkDeviceFilters, Fleet } from '../../../types/api';
import type { DeviceStatusFilter } from '../hooks/use-device-list-state';

interface DeviceFiltersProps {
  searchInputRef: React.Ref<HTMLInputElement>;
  searchQuery: string;
  onSearchQueryChange: (value: string) => void;
  filterStatus: DeviceStatusFilter;
  onFilterStatusChange: (value: DeviceStatusFilter) => void;
  filterFleetId: number | null;
  onFilterFleetIdChange: (value: number | null) => void;
  fleets: Fleet[];
  statusCounts: Record<DeviceStatusFilter, number>;
  hasSelection: boolean;
  totalMatchingCount: number;
  visibleCount: number;
  currentFilters: BulkDeviceFilters;
}

export function DeviceFilters({
  searchInputRef,
  searchQuery,
  onSearchQueryChange,
  filterStatus,
  onFilterStatusChange,
  filterFleetId,
  onFilterFleetIdChange,
  fleets,
  statusCounts,
  hasSelection,
  totalMatchingCount,
  visibleCount,
  currentFilters,
}: DeviceFiltersProps) {
  return (
    <Card elevation={Elevation.ONE} className="devices-controls">
      <div
        className={`bulk-action-bar-overlay ${hasSelection ? 'bulk-action-bar-overlay--visible' : ''}`}
      >
        <BulkActionBar
          totalMatchingCount={totalMatchingCount}
          visibleCount={visibleCount}
          currentFilters={currentFilters}
        />
      </div>
      <div className={`controls-content ${hasSelection ? 'controls-content--hidden' : ''}`}>
        <div className="controls-row">
          <div className="search-section">
            <InputGroup
              inputRef={searchInputRef}
              leftIcon="search"
              placeholder="Search by name or type..."
              value={searchQuery}
              onChange={(e) => onSearchQueryChange(e.target.value)}
              fill
              rightElement={
                searchQuery ? (
                  <Button icon="cross" minimal onClick={() => onSearchQueryChange('')} />
                ) : undefined
              }
            />
          </div>

          <div className="filter-section">
            {(['all', 'online', 'offline'] as const).map((status) => (
              <button
                key={status}
                className={`filter-pill ${filterStatus === status ? 'active' : ''} ${status !== 'all' ? `pill-${status}` : ''}`}
                onClick={() => onFilterStatusChange(status)}
              >
                {status !== 'all' && <StatusLed status={status} />}
                <span className="pill-label">
                  {status === 'all' ? 'All' : status.charAt(0).toUpperCase() + status.slice(1)}
                </span>
                <span className="pill-count mono-data">{statusCounts[status]}</span>
              </button>
            ))}
          </div>
        </div>

        {fleets.length > 0 && (
          <div className="controls-row" style={{ marginTop: 10 }}>
            <div className="filter-section">
              <button
                className={`filter-pill ${filterFleetId === null ? 'active' : ''}`}
                onClick={() => onFilterFleetIdChange(null)}
              >
                <span className="pill-label">All Fleets</span>
              </button>
              {fleets.map((fleet) => (
                <button
                  key={fleet.id}
                  className={`filter-pill ${filterFleetId === fleet.id ? 'active' : ''}`}
                  onClick={() => onFilterFleetIdChange(fleet.id)}
                >
                  <span className="pill-label">{fleet.name}</span>
                  <span className="pill-count mono-data">{fleet.device_count}</span>
                </button>
              ))}
            </div>
          </div>
        )}
      </div>
    </Card>
  );
}
