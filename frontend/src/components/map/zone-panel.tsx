import { useState } from 'react';
import {
  Button,
  Dialog,
  DialogBody,
  DialogFooter,
  InputGroup,
  FormGroup,
  Tag,
  Alert,
  Popover,
} from '@blueprintjs/core';
import { useZones, useCreateZone, useDeleteZone, useUpdateZone } from '../../hooks/use-zones';
import { useConfirmShortcut } from '../../hooks/use-confirm-shortcut';
import type { Zone, CircleGeometry, PolygonGeometry } from '../../types/zones';
import L from 'leaflet';
import './zone-panel.css';

const ZONE_COLORS = [
  '#4A90D9',
  '#0F9960',
  '#D9822B',
  '#E76A6E',
  '#9F7AEA',
  '#00B5D8',
  '#D69E2E',
  '#ED64A6',
  '#5C7080',
];

const STATUS_COLORS: Record<string, string> = {
  online: '#43bf4d',
  offline: '#868686',
  warning: '#d4a017',
};

interface PendingZoneGeometry {
  geometry_type: 'circle' | 'polygon';
  geometry_json: CircleGeometry | PolygonGeometry;
}

export interface MapDevice {
  id: string;
  name: string;
  status: string;
  latest_latitude: number;
  latest_longitude: number;
  last_seen_at?: string | null;
}

type PanelTab = 'devices' | 'zones';

interface MapPanelProps {
  drawMode: boolean;
  onToggleDrawMode: () => void;
  onZoneClick: (zone: Zone) => void;
  onDeviceClick: (device: MapDevice) => void;
  hiddenZoneIds: Set<string>;
  onToggleZoneVisibility: (id: string) => void;
  devices: MapDevice[];
  collapsed?: boolean;
}

export function ZonePanel({
  drawMode,
  onToggleDrawMode,
  onZoneClick,
  onDeviceClick,
  hiddenZoneIds,
  onToggleZoneVisibility,
  devices,
  collapsed,
}: MapPanelProps) {
  const { data: zones = [] } = useZones();
  const createZone = useCreateZone();
  const updateZone = useUpdateZone();
  const deleteZoneMutation = useDeleteZone();

  const [activeTab, setActiveTab] = useState<PanelTab>('devices');
  const [nameDialogOpen, setNameDialogOpen] = useState(false);
  const [pendingGeometry, setPendingGeometry] = useState<PendingZoneGeometry | null>(null);
  const [zoneName, setZoneName] = useState('');
  const [zoneDescription, setZoneDescription] = useState('');
  const [zoneColor, setZoneColor] = useState(ZONE_COLORS[0]);
  const [deleteAlertZone, setDeleteAlertZone] = useState<Zone | null>(null);

  const handleSaveZone = () => {
    if (!pendingGeometry || !zoneName.trim()) return;
    createZone.mutate(
      {
        name: zoneName.trim(),
        description: zoneDescription.trim() || undefined,
        geometry_type: pendingGeometry.geometry_type,
        geometry_json: pendingGeometry.geometry_json,
        color: zoneColor,
      },
      {
        onSuccess: () => {
          setNameDialogOpen(false);
          setPendingGeometry(null);
          setZoneName('');
          setZoneDescription('');
          setZoneColor(ZONE_COLORS[0]);
        },
      },
    );
  };

  useConfirmShortcut({
    isOpen: nameDialogOpen,
    canConfirm: !!zoneName.trim() && !createZone.isPending,
    onConfirm: handleSaveZone,
  });

  const handleChangeZoneColor = (zone: Zone, color: string) => {
    updateZone.mutate({
      id: zone.id,
      body: {
        name: zone.name,
        geometry_type: zone.geometry_type,
        geometry_json: zone.geometry_json,
        color,
      },
    });
  };

  const handleDeleteZone = () => {
    if (!deleteAlertZone) return;
    deleteZoneMutation.mutate(deleteAlertZone.id, {
      onSuccess: () => setDeleteAlertZone(null),
    });
  };

  const acceptDrawnLayer = (layer: L.Layer, type: string) => {
    let geometry_type: 'circle' | 'polygon';
    let geometry_json: CircleGeometry | PolygonGeometry;

    if (type === 'circle') {
      const circle = layer as L.Circle;
      const center = circle.getLatLng();
      geometry_type = 'circle';
      geometry_json = {
        center: [center.lat, center.lng],
        radius_meters: circle.getRadius(),
      };
    } else {
      const polygon = layer as L.Polygon;
      const latlngs = polygon.getLatLngs()[0] as L.LatLng[];
      geometry_type = 'polygon';
      geometry_json = {
        points: latlngs.map((ll) => [ll.lat, ll.lng] as [number, number]),
      };
    }

    setPendingGeometry({ geometry_type, geometry_json });
    setNameDialogOpen(true);
    onToggleDrawMode();
  };

  ZonePanel.acceptDrawnLayer = acceptDrawnLayer;

  return (
    <div
      className={`map-panel${collapsed ? ' map-panel--collapsed' : ''}`}
      data-focus-region={collapsed ? undefined : 'aside'}
      tabIndex={collapsed ? undefined : -1}
    >
      {/* Tab bar */}
      <div className="map-panel-tabs">
        <button
          className={`map-panel-tab${activeTab === 'devices' ? ' map-panel-tab--active' : ''}`}
          onClick={() => setActiveTab('devices')}
        >
          Devices
          <span className="map-panel-tab-count">{devices.length}</span>
        </button>
        <button
          className={`map-panel-tab${activeTab === 'zones' ? ' map-panel-tab--active' : ''}`}
          onClick={() => setActiveTab('zones')}
        >
          Zones
          <span className="map-panel-tab-count">{zones.length}</span>
        </button>
        {activeTab === 'zones' && (
          <Button
            small
            minimal
            intent={drawMode ? 'danger' : 'primary'}
            icon={drawMode ? 'cross' : 'plus'}
            className="map-panel-tab-action"
            onClick={onToggleDrawMode}
          />
        )}
      </div>

      {/* Devices tab */}
      {activeTab === 'devices' && (
        <div className="map-panel-list">
          {devices.map((device) => {
            const color = STATUS_COLORS[device.status] || STATUS_COLORS.offline;
            return (
              <div key={device.id} className="map-panel-row" onClick={() => onDeviceClick(device)}>
                <span
                  className="map-panel-led"
                  style={{ backgroundColor: color, boxShadow: `0 0 4px ${color}80` }}
                />
                <span className="map-panel-name">{device.name}</span>
                <span className="map-panel-status" style={{ color }}>
                  {device.status}
                </span>
              </div>
            );
          })}
          {devices.length === 0 && (
            <div className="map-panel-empty">
              <p>No devices with location data.</p>
            </div>
          )}
        </div>
      )}

      {/* Zones tab */}
      {activeTab === 'zones' && (
        <div className="map-panel-list">
          {zones.map((zone) => {
            const hidden = hiddenZoneIds.has(zone.id);
            return (
              <div
                key={zone.id}
                className={`map-panel-row${hidden ? ' map-panel-row--hidden' : ''}`}
                onClick={() => onZoneClick(zone)}
              >
                <Button
                  minimal
                  small
                  icon={hidden ? 'eye-off' : 'eye-open'}
                  className="map-panel-visibility"
                  onClick={(e) => {
                    e.stopPropagation();
                    onToggleZoneVisibility(zone.id);
                  }}
                />
                <Popover
                  content={
                    <div className="zone-color-picker" onClick={(e) => e.stopPropagation()}>
                      {ZONE_COLORS.map((c) => (
                        <button
                          key={c}
                          className={`zone-color-swatch${c === zone.color ? ' zone-color-swatch--active' : ''}`}
                          style={{ backgroundColor: c }}
                          onClick={() => handleChangeZoneColor(zone, c)}
                        />
                      ))}
                    </div>
                  }
                  placement="bottom-start"
                  minimal
                >
                  <span
                    className="map-panel-dot map-panel-dot--clickable"
                    style={{ backgroundColor: zone.color }}
                    onClick={(e) => e.stopPropagation()}
                  />
                </Popover>
                <span className="map-panel-name">{zone.name}</span>
                <Tag minimal className="map-panel-tag">
                  {zone.geometry_type}
                </Tag>
                <Button
                  minimal
                  small
                  icon="trash"
                  intent="danger"
                  className="map-panel-delete"
                  onClick={(e) => {
                    e.stopPropagation();
                    setDeleteAlertZone(zone);
                  }}
                />
              </div>
            );
          })}
          {zones.length === 0 && !drawMode && (
            <div className="map-panel-empty">
              <p>No zones defined yet.</p>
              <p>
                Click <strong>+</strong> to draw one on the map.
              </p>
            </div>
          )}
          {drawMode && (
            <div className="map-panel-draw-hint">
              Draw a shape on the map using the toolbar, then name it.
            </div>
          )}
        </div>
      )}

      {/* Footer */}
      <div className="map-panel-footer">
        {activeTab === 'devices' && (
          <span>
            {devices.length} device{devices.length !== 1 ? 's' : ''} on map
          </span>
        )}
        {activeTab === 'zones' && (
          <>
            <span>
              {zones.length} zone{zones.length !== 1 ? 's' : ''}
            </span>
            {hiddenZoneIds.size > 0 && (
              <span className="map-panel-footer-muted">{hiddenZoneIds.size} hidden</span>
            )}
          </>
        )}
      </div>

      {/* Zone create dialog */}
      <Dialog
        isOpen={nameDialogOpen}
        onClose={() => setNameDialogOpen(false)}
        title="Name Your Zone"
        className="bp6-dark"
      >
        <DialogBody>
          <FormGroup label="Name">
            <InputGroup
              value={zoneName}
              onChange={(e) => setZoneName(e.target.value)}
              placeholder="e.g. Warehouse A"
              autoFocus
            />
          </FormGroup>
          <FormGroup label="Description (optional)">
            <InputGroup
              value={zoneDescription}
              onChange={(e) => setZoneDescription(e.target.value)}
              placeholder="Optional description"
            />
          </FormGroup>
          <FormGroup label="Color">
            <div className="zone-color-picker">
              {ZONE_COLORS.map((c) => (
                <button
                  key={c}
                  className={`zone-color-swatch${c === zoneColor ? ' zone-color-swatch--active' : ''}`}
                  style={{ backgroundColor: c }}
                  onClick={() => setZoneColor(c)}
                />
              ))}
            </div>
          </FormGroup>
        </DialogBody>
        <DialogFooter
          actions={
            <>
              <Button onClick={() => setNameDialogOpen(false)}>Cancel</Button>
              <Button
                intent="primary"
                onClick={handleSaveZone}
                disabled={!zoneName.trim()}
                loading={createZone.isPending}
              >
                Save Zone
              </Button>
            </>
          }
        />
      </Dialog>

      {/* Zone delete confirmation */}
      <Alert
        isOpen={!!deleteAlertZone}
        onClose={() => setDeleteAlertZone(null)}
        onConfirm={handleDeleteZone}
        intent="danger"
        confirmButtonText="Delete"
        cancelButtonText="Cancel"
      >
        <p>Delete zone "{deleteAlertZone?.name}"? This cannot be undone.</p>
      </Alert>
    </div>
  );
}

ZonePanel.acceptDrawnLayer = (_layer: L.Layer, _type: string): void => {};
