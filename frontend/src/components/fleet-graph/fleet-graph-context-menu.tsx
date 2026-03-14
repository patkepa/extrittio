import { useEffect, useRef } from 'react';
import { Menu, MenuItem, MenuDivider } from '@blueprintjs/core';
import { createPortal } from 'react-dom';
import { useFleets } from '../../hooks/use-fleets';
import { useSelectionStore } from '../../stores/selection-store';
import type { GraphNode } from './build-force-graph-data';

interface ContextMenuTarget {
  type: 'device' | 'fleet';
  node: GraphNode;
}

export interface ContextMenuState {
  position: { x: number; y: number };
  target: ContextMenuTarget;
}

interface FleetGraphContextMenuProps {
  state: ContextMenuState;
  onClose: () => void;
  onViewDetails: (node: GraphNode) => void;
  onAssignFleet: (deviceIds: string[], fleetId: number) => void;
  onRemoveFromFleet: (deviceIds: string[]) => void;
}

export const FleetGraphContextMenu = ({
  state,
  onClose,
  onViewDetails,
  onAssignFleet,
  onRemoveFromFleet,
}: FleetGraphContextMenuProps) => {
  const { data: fleets = [] } = useFleets();
  const { selectedDeviceIds, toggleDevice, addToSelection, removeFromSelection } =
    useSelectionStore();

  const menuRef = useRef<HTMLDivElement>(null);

  // Focus the menu container on mount for keyboard navigation
  useEffect(() => {
    menuRef.current?.focus();
  }, []);

  const { position, target } = state;
  const isSelected = selectedDeviceIds.has(target.node.id);
  const hasMultiSelection = isSelected && selectedDeviceIds.size > 1;

  // Determine which device IDs an action applies to
  const actionDeviceIds = hasMultiSelection ? Array.from(selectedDeviceIds) : [target.node.id];

  const handleClickOutside = (e: React.MouseEvent) => {
    e.stopPropagation();
    onClose();
  };

  if (target.type === 'fleet') {
    const fleetDeviceIds = target.node.neighbors
      .filter((n) => n.type === 'device')
      .map((n) => n.id);

    return createPortal(
      <div
        className="fleet-graph-context-menu-backdrop"
        onClick={handleClickOutside}
        onContextMenu={(e) => {
          e.preventDefault();
          onClose();
        }}
      >
        <div
          ref={menuRef}
          className="fleet-graph-context-menu"
          style={{ left: position.x, top: position.y }}
          onClick={(e) => e.stopPropagation()}
          tabIndex={-1}
        >
          <Menu>
            <MenuItem
              icon="select"
              text={`Select all ${fleetDeviceIds.length} devices`}
              onClick={() => {
                addToSelection(fleetDeviceIds);
                onClose();
              }}
            />
            <MenuItem
              icon="disable"
              text="Deselect all devices"
              onClick={() => {
                removeFromSelection(fleetDeviceIds);
                onClose();
              }}
            />
          </Menu>
        </div>
      </div>,
      document.body,
    );
  }

  // Device context menu
  return createPortal(
    <div
      className="fleet-graph-context-menu-backdrop"
      onClick={handleClickOutside}
      onContextMenu={(e) => {
        e.preventDefault();
        onClose();
      }}
    >
      <div
        ref={menuRef}
        className="fleet-graph-context-menu"
        style={{ left: position.x, top: position.y }}
        onClick={(e) => e.stopPropagation()}
        tabIndex={-1}
      >
        <Menu>
          <MenuItem
            icon="eye-open"
            text="View Details"
            onClick={() => {
              onViewDetails(target.node);
              onClose();
            }}
          />
          <MenuDivider />
          <MenuItem icon="flows" text="Assign to Fleet">
            {fleets.map((fleet) => (
              <MenuItem
                key={fleet.id}
                text={fleet.name}
                onClick={() => {
                  onAssignFleet(actionDeviceIds, fleet.id);
                  onClose();
                }}
              />
            ))}
            {fleets.length > 0 && <MenuDivider />}
            <MenuItem
              text="Remove from fleet"
              icon="cross"
              intent="warning"
              onClick={() => {
                onRemoveFromFleet(actionDeviceIds);
                onClose();
              }}
            />
          </MenuItem>
          <MenuDivider />
          <MenuItem
            icon={isSelected ? 'disable' : 'select'}
            text={isSelected ? 'Deselect' : 'Select'}
            onClick={() => {
              toggleDevice(target.node.id);
              onClose();
            }}
          />
        </Menu>
      </div>
    </div>,
    document.body,
  );
};
