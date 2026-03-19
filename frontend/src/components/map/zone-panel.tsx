import { useState } from "react";
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
} from "@blueprintjs/core";
import { useZones, useCreateZone, useDeleteZone, useUpdateZone } from "../../hooks/use-zones";
import type { Zone, CircleGeometry, PolygonGeometry } from "../../types/zones";
import L from "leaflet";
import "./zone-panel.css";

const ZONE_COLORS = [
  "#4A90D9",
  "#0F9960",
  "#D9822B",
  "#E76A6E",
  "#9F7AEA",
  "#00B5D8",
  "#D69E2E",
  "#ED64A6",
  "#5C7080",
];

interface PendingZoneGeometry {
  geometry_type: "circle" | "polygon";
  geometry_json: CircleGeometry | PolygonGeometry;
}

interface ZonePanelProps {
  drawMode: boolean;
  onToggleDrawMode: () => void;
  onZoneClick: (zone: Zone) => void;
  hiddenZoneIds: Set<string>;
  onToggleZoneVisibility: (id: string) => void;
  collapsed?: boolean;
}

export function ZonePanel({
  drawMode,
  onToggleDrawMode,
  onZoneClick,
  hiddenZoneIds,
  onToggleZoneVisibility,
  collapsed,
}: ZonePanelProps) {
  const { data: zones = [] } = useZones();
  const createZone = useCreateZone();
  const updateZone = useUpdateZone();
  const deleteZoneMutation = useDeleteZone();

  const [nameDialogOpen, setNameDialogOpen] = useState(false);
  const [pendingGeometry, setPendingGeometry] = useState<PendingZoneGeometry | null>(null);
  const [zoneName, setZoneName] = useState("");
  const [zoneDescription, setZoneDescription] = useState("");
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
          setZoneName("");
          setZoneDescription("");
          setZoneColor(ZONE_COLORS[0]);
        },
      }
    );
  };

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
    let geometry_type: "circle" | "polygon";
    let geometry_json: CircleGeometry | PolygonGeometry;

    if (type === "circle") {
      const circle = layer as L.Circle;
      const center = circle.getLatLng();
      geometry_type = "circle";
      geometry_json = {
        center: [center.lat, center.lng],
        radius_meters: circle.getRadius(),
      };
    } else {
      const polygon = layer as L.Polygon;
      const latlngs = polygon.getLatLngs()[0] as L.LatLng[];
      geometry_type = "polygon";
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
    <div className={`zone-panel${collapsed ? " zone-panel--collapsed" : ""}`}>
      <div className="zone-panel-header">
        <span className="zone-panel-title">Zones</span>
        <Button
          small
          minimal
          intent={drawMode ? "danger" : "primary"}
          icon={drawMode ? "cross" : "plus"}
          onClick={onToggleDrawMode}
        >
          {drawMode ? "Cancel" : "Create"}
        </Button>
      </div>

      <div className="zone-panel-list">
        {zones.map((zone) => {
          const hidden = hiddenZoneIds.has(zone.id);
          return (
            <div
              key={zone.id}
              className={`zone-panel-row${hidden ? " zone-panel-row--hidden" : ""}`}
              onClick={() => onZoneClick(zone)}
            >
              <Button
                minimal
                small
                icon={hidden ? "eye-off" : "eye-open"}
                className="zone-panel-visibility"
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
                        className={`zone-color-swatch${c === zone.color ? " zone-color-swatch--active" : ""}`}
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
                  className="zone-panel-dot zone-panel-dot--clickable"
                  style={{ backgroundColor: zone.color }}
                  onClick={(e) => e.stopPropagation()}
                />
              </Popover>
              <span className="zone-panel-name">{zone.name}</span>
              <Tag minimal className="zone-panel-tag">{zone.geometry_type}</Tag>
              <Button
                minimal
                small
                icon="trash"
                intent="danger"
                className="zone-panel-delete"
                onClick={(e) => {
                  e.stopPropagation();
                  setDeleteAlertZone(zone);
                }}
              />
            </div>
          );
        })}
        {zones.length === 0 && !drawMode && (
          <div className="zone-panel-empty">
            <p>No zones defined yet.</p>
            <p>Click <strong>Create</strong> to draw one on the map.</p>
          </div>
        )}
        {drawMode && (
          <div className="zone-panel-draw-hint">
            Draw a shape on the map using the toolbar, then name it.
          </div>
        )}
      </div>

      <div className="zone-panel-footer">
        <span>{zones.length} zone{zones.length !== 1 ? "s" : ""}</span>
        {hiddenZoneIds.size > 0 && (
          <span className="zone-panel-footer-hidden">
            {hiddenZoneIds.size} hidden
          </span>
        )}
      </div>

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
                  className={`zone-color-swatch${c === zoneColor ? " zone-color-swatch--active" : ""}`}
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
