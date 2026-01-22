import { useState } from 'react';
import {
  Card,
  Elevation,
  H3,
  HTMLTable,
  Tag,
  Button,
  InputGroup,
  ButtonGroup,
  Icon,
  Drawer,
  Position,
  H4,
  FormGroup,
  Intent,
  Callout,
  Divider,
} from '@blueprintjs/core';
import './devices.css';

interface Device {
  id: string;
  name: string;
  type: string;
  status: 'online' | 'offline' | 'warning';
  lastSeen: string;
  firmware: string;
  location: string;
  uptime: string;
}

export const Devices = () => {
  const [searchQuery, setSearchQuery] = useState('');
  const [filterStatus, setFilterStatus] = useState<string>('all');
  const [selectedDevice, setSelectedDevice] = useState<Device | null>(null);
  const [isDrawerOpen, setIsDrawerOpen] = useState(false);

  const devices: Device[] = [
    {
      id: '1',
      name: 'Temperature Sensor 01',
      type: 'Sensor',
      status: 'online',
      lastSeen: '2 minutes ago',
      firmware: 'v2.1.3',
      location: 'Building A - Floor 2',
      uptime: '45 days',
    },
    {
      id: '2',
      name: 'Smart Camera 03',
      type: 'Camera',
      status: 'online',
      lastSeen: '5 minutes ago',
      firmware: 'v1.8.2',
      location: 'Entrance - Main Gate',
      uptime: '12 days',
    },
    {
      id: '3',
      name: 'Motion Detector 12',
      type: 'Sensor',
      status: 'offline',
      lastSeen: '2 hours ago',
      firmware: 'v2.0.1',
      location: 'Warehouse - Zone C',
      uptime: '0 days',
    },
    {
      id: '4',
      name: 'Humidity Sensor 05',
      type: 'Sensor',
      status: 'warning',
      lastSeen: '1 minute ago',
      firmware: 'v2.1.1',
      location: 'Server Room',
      uptime: '89 days',
    },
    {
      id: '5',
      name: 'Smart Lock 08',
      type: 'Actuator',
      status: 'online',
      lastSeen: '30 seconds ago',
      firmware: 'v3.0.0',
      location: 'Office - Room 204',
      uptime: '156 days',
    },
  ];

  const getStatusIntent = (status: string): Intent => {
    switch (status) {
      case 'online':
        return 'success';
      case 'offline':
        return 'danger';
      case 'warning':
        return 'warning';
      default:
        return 'none';
    }
  };

  const getStatusIcon = (status: string) => {
    switch (status) {
      case 'online':
        return 'tick-circle';
      case 'offline':
        return 'delete';
      case 'warning':
        return 'warning-sign';
      default:
        return 'help';
    }
  };

  const filteredDevices = devices.filter((device) => {
    const matchesSearch =
      device.name.toLowerCase().includes(searchQuery.toLowerCase()) ||
      device.type.toLowerCase().includes(searchQuery.toLowerCase()) ||
      device.location.toLowerCase().includes(searchQuery.toLowerCase());

    const matchesStatus = filterStatus === 'all' || device.status === filterStatus;

    return matchesSearch && matchesStatus;
  });

  const handleViewDevice = (device: Device) => {
    setSelectedDevice(device);
    setIsDrawerOpen(true);
  };

  const handleDeleteDevice = (device: Device) => {
    // eslint-disable-next-line no-alert
    if (window.confirm(`Are you sure you want to delete ${device.name}?`)) {
      // Handle delete
      console.log('Delete device:', device.id);
    }
  };

  const statusCounts = {
    all: devices.length,
    online: devices.filter((d) => d.status === 'online').length,
    offline: devices.filter((d) => d.status === 'offline').length,
    warning: devices.filter((d) => d.status === 'warning').length,
  };

  return (
    <div className="devices-page">
      {/* Header */}
      <div className="page-header">
        <div>
          <H3>Devices</H3>
          <p className="page-description">
            Manage and monitor your IoT devices ({filteredDevices.length} of {devices.length})
          </p>
        </div>
        <Button intent="primary" icon="add" large>
          Add Device
        </Button>
      </div>

      {/* Filters and Search */}
      <Card elevation={Elevation.ONE} className="devices-controls">
        <div className="controls-row">
          <div className="search-section">
            <InputGroup
              leftIcon="search"
              placeholder="Search devices by name, type, or location..."
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              large
              fill
              rightElement={
                searchQuery ? (
                  <Button icon="cross" minimal onClick={() => setSearchQuery('')} />
                ) : undefined
              }
            />
          </div>

          <div className="filter-section">
            <ButtonGroup large>
              <Button
                active={filterStatus === 'all'}
                onClick={() => setFilterStatus('all')}
                text={`All (${statusCounts.all})`}
              />
              <Button
                active={filterStatus === 'online'}
                onClick={() => setFilterStatus('online')}
                intent="success"
                text={`Online (${statusCounts.online})`}
              />
              <Button
                active={filterStatus === 'offline'}
                onClick={() => setFilterStatus('offline')}
                intent="danger"
                text={`Offline (${statusCounts.offline})`}
              />
              <Button
                active={filterStatus === 'warning'}
                onClick={() => setFilterStatus('warning')}
                intent="warning"
                text={`Warning (${statusCounts.warning})`}
              />
            </ButtonGroup>
          </div>
        </div>
      </Card>

      {/* Devices Table */}
      <Card elevation={Elevation.TWO} className="devices-card">
        {filteredDevices.length === 0 ? (
          <div className="empty-state">
            <Icon icon="search" size={48} />
            <H4>No devices found</H4>
            <p>Try adjusting your search or filter criteria</p>
          </div>
        ) : (
          <HTMLTable striped interactive className="devices-table">
            <thead>
              <tr>
                <th>Status</th>
                <th>Name</th>
                <th>Type</th>
                <th>Location</th>
                <th>Last Seen</th>
                <th>Firmware</th>
                <th>Uptime</th>
                <th className="actions-column">Actions</th>
              </tr>
            </thead>
            <tbody>
              {filteredDevices.map((device) => (
                <tr key={device.id}>
                  <td>
                    <div className="status-cell">
                      <Icon
                        icon={getStatusIcon(device.status)}
                        intent={getStatusIntent(device.status)}
                        size={16}
                      />
                    </div>
                  </td>
                  <td>
                    <strong>{device.name}</strong>
                  </td>
                  <td>
                    <Tag minimal>{device.type}</Tag>
                  </td>
                  <td>{device.location}</td>
                  <td>{device.lastSeen}</td>
                  <td>
                    <code className="firmware-badge">{device.firmware}</code>
                  </td>
                  <td>{device.uptime}</td>
                  <td className="actions-column">
                    <ButtonGroup minimal>
                      <Button
                        icon="eye-open"
                        small
                        onClick={() => handleViewDevice(device)}
                        title="View Details"
                      />
                      <Button icon="edit" small title="Edit Device" />
                      <Button
                        icon="trash"
                        small
                        intent="danger"
                        onClick={() => handleDeleteDevice(device)}
                        title="Delete Device"
                      />
                    </ButtonGroup>
                  </td>
                </tr>
              ))}
            </tbody>
          </HTMLTable>
        )}
      </Card>

      {/* Device Detail Drawer */}
      <Drawer
        icon="info-sign"
        title="Device Details"
        isOpen={isDrawerOpen}
        onClose={() => setIsDrawerOpen(false)}
        position={Position.RIGHT}
        size="500px"
      >
        <div className="drawer-content">
          {selectedDevice && (
            <>
              <div className="drawer-header">
                <div className="device-status-banner">
                  <Icon
                    icon={getStatusIcon(selectedDevice.status)}
                    intent={getStatusIntent(selectedDevice.status)}
                    size={20}
                  />
                  <div>
                    <H4 style={{ margin: 0 }}>{selectedDevice.name}</H4>
                    <p style={{ margin: 0, opacity: 0.7 }}>
                      {selectedDevice.type} • {selectedDevice.status}
                    </p>
                  </div>
                </div>
              </div>

              <Divider />

              <div className="drawer-body">
                {selectedDevice.status === 'offline' && (
                  <Callout intent="danger" icon="error" style={{ marginBottom: 20 }}>
                    This device has been offline for {selectedDevice.lastSeen}
                  </Callout>
                )}

                {selectedDevice.status === 'warning' && (
                  <Callout intent="warning" icon="warning-sign" style={{ marginBottom: 20 }}>
                    This device is reporting warnings. Check telemetry data.
                  </Callout>
                )}

                <FormGroup label="Device ID">
                  <InputGroup value={selectedDevice.id} readOnly />
                </FormGroup>

                <FormGroup label="Location">
                  <InputGroup value={selectedDevice.location} readOnly />
                </FormGroup>

                <FormGroup label="Firmware Version">
                  <InputGroup value={selectedDevice.firmware} readOnly />
                </FormGroup>

                <FormGroup label="Last Seen">
                  <InputGroup value={selectedDevice.lastSeen} readOnly />
                </FormGroup>

                <FormGroup label="Uptime">
                  <InputGroup value={selectedDevice.uptime} readOnly />
                </FormGroup>

                <Divider />

                <H4>Quick Actions</H4>
                <div className="quick-actions">
                  <Button icon="refresh" fill>
                    Restart Device
                  </Button>
                  <Button icon="cloud-upload" fill>
                    Update Firmware
                  </Button>
                  <Button icon="chart" fill>
                    View Telemetry
                  </Button>
                  <Button icon="cog" fill>
                    Configure
                  </Button>
                </div>
              </div>

              <div className="drawer-footer">
                <Button onClick={() => setIsDrawerOpen(false)}>Close</Button>
                <Button intent="primary" icon="edit">
                  Edit Device
                </Button>
              </div>
            </>
          )}
        </div>
      </Drawer>
    </div>
  );
};
