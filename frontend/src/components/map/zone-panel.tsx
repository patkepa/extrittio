import { useState } from "react";
import {
  Button,
  Card,
  Dialog,
  DialogBody,
  DialogFooter,
  InputGroup,
  FormGroup,
  Tag,
  Alert,
} from "@blueprintjs/core";
import { useZones, useCreateZone, useDeleteZone } from "../../hooks/use-zones";
import type { Zone, CircleGeometry, PolygonGeometry } from "../../types/zones";
import L from "leaflet";
import "./zone-panel.css";

interface PendingZoneGeometry {
  geometry_type: "circle" | "polygon";
  geometry_json: CircleGeometry | PolygonGeometry;
}

interface ZonePanelProps {
  drawMode: boolean;
  onToggleDrawMode: () => void;
  onDrawCreated: (layer: L.Layer, type: string) => void;
}

export function ZonePanel({ drawMode, onToggleDrawMode }: ZonePanelProps) {
  const { data: zones = [] } = useZones();
  const createZone = useCreateZone();
  const deleteZoneMutation = useDeleteZone();

  const [nameDialogOpen, setNameDialogOpen] = useState(false);
  const [pendingGeometry, setPendingGeometry] = useState<PendingZoneGeometry | null>(null);
  const [zoneName, setZoneName] = useState("");
  const [zoneDescription, setZoneDescription] = useState("");
  const [deleteAlertZone, setDeleteAlertZone] = useState<Zone | null>(null);

  const handleSaveZone = () => {
    if (!pendingGeometry || !zoneName.trim()) return;
    createZone.mutate(
      {
        name: zoneName.trim(),
        description: zoneDescription.trim(),
        geometry_type: pendingGeometry.geometry_type,
        geometry_json: pendingGeometry.geometry_json,
      },
      {
        onSuccess: () => {
          setNameDialogOpen(false);
          setPendingGeometry(null);
          setZoneName("");
          setZoneDescription("");
        },
      }
    );
  };

  const handleDeleteZone = () => {
    if (!deleteAlertZone) return;
    deleteZoneMutation.mutate(deleteAlertZone.id, {
      onSuccess: () => setDeleteAlertZone(null),
    });
  };

  /**
   * Called from MapPage via the ref when leaflet-draw creates a shape.
   * Extracts geometry, stores it in state, and opens the naming dialog.
   */
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
    onToggleDrawMode(); // exit draw mode after shape is drawn
  };

  // Expose acceptDrawnLayer so MapPage can call it after draw:created
  // (stored on the function component itself for easy access — no ref needed)
  ZonePanel.acceptDrawnLayer = acceptDrawnLayer;

  return (
    <div className="zone-panel">
      <div className="zone-panel-header">
        <span className="section-label">ZONES</span>
        <Button
          small
          intent={drawMode ? "danger" : "primary"}
          icon={drawMode ? "cross" : "plus"}
          onClick={onToggleDrawMode}
        >
          {drawMode ? "Cancel" : "Create Zone"}
        </Button>
      </div>

      <div className="zone-panel-list">
        {zones.map((zone) => (
          <Card key={zone.id} className="zone-panel-item" compact interactive={false}>
            <div className="zone-panel-item-header">
              <span
                className="zone-color-dot"
                style={{ backgroundColor: zone.color }}
              />
              <span className="zone-panel-item-name">{zone.name}</span>
              <Tag minimal>{zone.geometry_type}</Tag>
            </div>
            {zone.description && (
              <p className="zone-panel-item-desc">{zone.description}</p>
            )}
            <Button
              small
              minimal
              intent="danger"
              icon="trash"
              onClick={() => setDeleteAlertZone(zone)}
            />
          </Card>
        ))}
        {zones.length === 0 && (
          <p className="zone-panel-empty">No zones defined. Click "Create Zone" to add one.</p>
        )}
      </div>

      <Dialog
        isOpen={nameDialogOpen}
        onClose={() => setNameDialogOpen(false)}
        title="Name Your Zone"
        className="bp5-dark"
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

/**
 * Stable reference updated on each render so MapPage can route draw events
 * into the panel's internal state without needing an imperative ref.
 */
ZonePanel.acceptDrawnLayer = (_layer: L.Layer, _type: string): void => {};
