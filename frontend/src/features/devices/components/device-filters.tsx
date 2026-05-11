import { Card, Elevation } from '@blueprintjs/core';
import { BulkActionBar } from '../../../components/devices/bulk-action-bar';
import { FilterPill, SearchField } from '@patkepa/ui';
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
            <SearchField
              inputRef={searchInputRef}
              placeholder="Search by name or type..."
              value={searchQuery}
              onChange={onSearchQueryChange}
            />
          </div>

          <div className="filter-section">
            {(['all', 'online', 'offline'] as const).map((status) => (
              <FilterPill
                key={status}
                value={status}
                label={status === 'all' ? 'All' : status.charAt(0).toUpperCase() + status.slice(1)}
                active={filterStatus === status}
                className={status !== 'all' ? `pill-${status}` : undefined}
                status={status !== 'all' ? status : undefined}
                count={statusCounts[status]}
                onSelect={onFilterStatusChange}
              />
            ))}
          </div>
        </div>

        {fleets.length > 0 && (
          <div className="controls-row" style={{ marginTop: 10 }}>
            <div className="filter-section">
              <FilterPill
                value="all"
                label="All Fleets"
                active={filterFleetId === null}
                onSelect={() => onFilterFleetIdChange(null)}
              />
              {fleets.map((fleet) => (
                <FilterPill
                  key={fleet.id}
                  value={String(fleet.id)}
                  label={fleet.name}
                  active={filterFleetId === fleet.id}
                  count={fleet.device_count}
                  onSelect={() => onFilterFleetIdChange(fleet.id)}
                />
              ))}
            </div>
          </div>
        )}
      </div>
    </Card>
  );
}
