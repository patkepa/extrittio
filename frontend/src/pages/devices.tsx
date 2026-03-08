import { useState } from 'react';
import {
  Search,
  X,
  Plus,
  ChevronUp,
  ChevronDown,
  Eye,
  Pencil,
  Trash2,
  RefreshCw,
  CloudUpload,
  BarChart3,
  Settings,
  AlertCircle,
  AlertTriangle,
} from 'lucide-react';
import { AreaChart, Area, ResponsiveContainer } from 'recharts';
import { Card } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Badge } from '@/components/ui/badge';
import {
  Table,
  TableHead,
  TableBody,
  TableRow,
  TableHeader,
  TableCell,
} from '@/components/ui/table';
import {
  Drawer,
  DrawerContent,
  DrawerHeader,
  DrawerBody,
  DrawerFooter,
} from '@/components/ui/drawer';
import { Tabs, TabsList, TabsTrigger, TabsContent } from '@/components/ui/tabs';
import { Callout } from '@/components/ui/callout';

interface Device {
  id: string;
  name: string;
  type: string;
  status: 'online' | 'offline' | 'warning';
  lastSeen: string;
  firmware: string;
  location: string;
  uptime: string;
  activity: number[];
}

const devices: Device[] = [
  { id: 'DEV-001', name: 'Temperature Sensor 01', type: 'Sensor', status: 'online', lastSeen: '2 min ago', firmware: 'v2.1.3', location: 'Building A — Floor 2', uptime: '45d 12h', activity: [20, 35, 28, 45, 38, 52, 44] },
  { id: 'DEV-002', name: 'Smart Camera 03', type: 'Camera', status: 'online', lastSeen: '5 min ago', firmware: 'v1.8.2', location: 'Entrance — Main Gate', uptime: '12d 8h', activity: [50, 42, 55, 48, 60, 52, 58] },
  { id: 'DEV-003', name: 'Motion Detector 12', type: 'Sensor', status: 'offline', lastSeen: '2h ago', firmware: 'v2.0.1', location: 'Warehouse — Zone C', uptime: '0d', activity: [30, 25, 20, 15, 10, 5, 0] },
  { id: 'DEV-004', name: 'Humidity Sensor 05', type: 'Sensor', status: 'warning', lastSeen: '1 min ago', firmware: 'v2.1.1', location: 'Server Room', uptime: '89d 4h', activity: [40, 45, 60, 75, 80, 85, 90] },
  { id: 'DEV-005', name: 'Smart Lock 08', type: 'Actuator', status: 'online', lastSeen: '30s ago', firmware: 'v3.0.0', location: 'Office — Room 204', uptime: '156d 2h', activity: [10, 15, 12, 18, 14, 20, 16] },
];

type SortField = 'name' | 'status' | 'lastSeen' | 'uptime';
type SortDir = 'asc' | 'desc';

const telemetryData = {
  cpu: [32, 45, 38, 42, 55, 48, 52, 44, 40, 38, 42, 50],
  memory: [60, 62, 58, 65, 63, 67, 64, 68, 62, 60, 65, 63],
  signal: [85, 82, 88, 84, 90, 86, 88, 85, 83, 87, 89, 85],
};

const logEntries = [
  { time: '14:32:01', level: 'INFO', message: 'Device heartbeat received' },
  { time: '14:30:45', level: 'INFO', message: 'Telemetry data uploaded (128 bytes)' },
  { time: '14:28:12', level: 'WARN', message: 'Signal strength below threshold' },
  { time: '14:25:00', level: 'INFO', message: 'Configuration sync completed' },
  { time: '14:20:33', level: 'ERROR', message: 'Connection timeout — retrying' },
  { time: '14:18:15', level: 'INFO', message: 'Firmware check: up to date' },
];

const configEntries = [
  { key: 'reporting_interval', value: '30s' },
  { key: 'max_retries', value: '3' },
  { key: 'protocol', value: 'MQTT v5' },
  { key: 'encryption', value: 'TLS 1.3' },
  { key: 'data_format', value: 'Protobuf' },
  { key: 'keepalive', value: '60s' },
];

const logLevelClasses: Record<string, string> = {
  info: 'text-accent bg-accent-muted',
  warn: 'text-warning bg-warning-muted',
  error: 'text-danger bg-danger-muted',
};

export const Devices = () => {
  const [searchQuery, setSearchQuery] = useState('');
  const [filterStatus, setFilterStatus] = useState<string>('all');
  const [selectedDevice, setSelectedDevice] = useState<Device | null>(null);
  const [isDrawerOpen, setIsDrawerOpen] = useState(false);
  const [sortField, setSortField] = useState<SortField>('name');
  const [sortDir, setSortDir] = useState<SortDir>('asc');
  const [drawerTab, setDrawerTab] = useState('overview');

  const filteredDevices = devices
    .filter((device) => {
      const matchesSearch =
        device.name.toLowerCase().includes(searchQuery.toLowerCase()) ||
        device.type.toLowerCase().includes(searchQuery.toLowerCase()) ||
        device.location.toLowerCase().includes(searchQuery.toLowerCase());
      const matchesStatus = filterStatus === 'all' || device.status === filterStatus;
      return matchesSearch && matchesStatus;
    })
    .sort((a, b) => {
      const dir = sortDir === 'asc' ? 1 : -1;
      if (sortField === 'name') return a.name.localeCompare(b.name) * dir;
      if (sortField === 'status') return a.status.localeCompare(b.status) * dir;
      return 0;
    });

  const handleSort = (field: SortField) => {
    if (sortField === field) {
      setSortDir(sortDir === 'asc' ? 'desc' : 'asc');
    } else {
      setSortField(field);
      setSortDir('asc');
    }
  };

  const handleViewDevice = (device: Device) => {
    setSelectedDevice(device);
    setDrawerTab('overview');
    setIsDrawerOpen(true);
  };

  const statusCounts = {
    all: devices.length,
    online: devices.filter((d) => d.status === 'online').length,
    offline: devices.filter((d) => d.status === 'offline').length,
    warning: devices.filter((d) => d.status === 'warning').length,
  };

  const SortHeader = ({ field, children }: { field: SortField; children: React.ReactNode }) => (
    <TableHeader
      className="cursor-pointer select-none hover:text-foreground"
      onClick={() => handleSort(field)}
    >
      <span className="flex items-center gap-1">
        {children}
        {sortField === field && (
          sortDir === 'asc' ? <ChevronUp size={12} /> : <ChevronDown size={12} />
        )}
      </span>
    </TableHeader>
  );

  const getStatusColor = (status: string) => {
    switch (status) {
      case 'online': return '#0F9960';
      case 'offline': return '#E76A6E';
      case 'warning': return '#D99E0B';
      default: return '#888';
    }
  };

  return (
    <div className="max-w-[1600px] mx-auto">
      {/* Header */}
      <div className="flex justify-between items-start mb-6">
        <div>
          <h3 className="mb-1 text-[28px] font-semibold tracking-tight text-foreground">
            Devices
          </h3>
          <p className="text-muted text-[13px] font-medium">
            {filteredDevices.length} of {devices.length} devices
          </p>
        </div>
        <Button variant="primary">
          <Plus size={14} />
          Add Device
        </Button>
      </div>

      {/* Filters and Search */}
      <Card className="p-4 px-5 mb-4">
        <div className="flex gap-4 items-center max-lg:flex-col max-lg:items-stretch">
          <div className="flex-1 min-w-[250px] max-lg:min-w-full">
            <Input
              leftIcon={<Search size={14} />}
              placeholder="Search by name, type, or location..."
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              rightElement={
                searchQuery ? (
                  <button
                    className="text-muted hover:text-foreground transition-colors p-1"
                    onClick={() => setSearchQuery('')}
                  >
                    <X size={14} />
                  </button>
                ) : undefined
              }
            />
          </div>

          <div className="flex gap-2 shrink-0 max-lg:flex-wrap">
            {(['all', 'online', 'offline', 'warning'] as const).map((status) => (
              <button
                key={status}
                className={`flex items-center gap-1.5 px-3.5 py-1.5 rounded-full text-[13px] font-semibold border transition-all duration-150 cursor-pointer ${
                  filterStatus === status
                    ? 'bg-accent border-accent text-white'
                    : 'bg-transparent border-border text-muted hover:border-accent/40 hover:text-foreground'
                }`}
                onClick={() => setFilterStatus(status)}
              >
                {status !== 'all' && (
                  <span
                    className={`status-led status-led--${status} ${
                      filterStatus === status ? '!shadow-none' : ''
                    }`}
                  />
                )}
                <span>{status === 'all' ? 'All' : status.charAt(0).toUpperCase() + status.slice(1)}</span>
                <span className="font-mono text-[11px] opacity-80">{statusCounts[status]}</span>
              </button>
            ))}
          </div>
        </div>
      </Card>

      {/* Devices Table */}
      <Card className="p-0 overflow-hidden">
        {filteredDevices.length === 0 ? (
          <div className="flex flex-col items-center justify-center py-20 px-5 text-center">
            <Search size={48} className="text-muted opacity-30 mb-4" />
            <h4 className="font-semibold mb-2">No devices found</h4>
            <p className="text-muted text-[13px]">Try adjusting your search or filter criteria</p>
          </div>
        ) : (
          <Table>
            <TableHead>
              <TableRow className="border-t-0 hover:bg-transparent">
                <TableHeader style={{ width: 40 }} />
                <SortHeader field="name">Name</SortHeader>
                <TableHeader>Type</TableHeader>
                <TableHeader>Location</TableHeader>
                <TableHeader>Last Seen</TableHeader>
                <TableHeader>Firmware</TableHeader>
                <TableHeader style={{ width: 80 }}>Activity</TableHeader>
                <TableHeader>Uptime</TableHeader>
                <TableHeader className="w-[120px] text-right">Actions</TableHeader>
              </TableRow>
            </TableHead>
            <TableBody>
              {filteredDevices.map((device, idx) => (
                <TableRow
                  key={device.id}
                  className={`cursor-pointer transition-colors stagger-item ${
                    selectedDevice?.id === device.id
                      ? 'bg-accent-muted border-l-[3px] border-l-accent'
                      : 'even:bg-white/[0.015]'
                  }`}
                  onClick={() => handleViewDevice(device)}
                  style={{ animationDelay: `${idx * 30}ms` }}
                >
                  <TableCell>
                    <span className={`status-led status-led--${device.status}`} />
                  </TableCell>
                  <TableCell>
                    <div className="flex flex-col gap-px">
                      <span className="text-[13px] font-semibold text-foreground">{device.name}</span>
                      <span className="text-[10px] text-muted font-mono">{device.id}</span>
                    </div>
                  </TableCell>
                  <TableCell>
                    <Badge>{device.type}</Badge>
                  </TableCell>
                  <TableCell className="text-muted text-[13px]">{device.location}</TableCell>
                  <TableCell>
                    <span className="font-mono text-[13px]">{device.lastSeen}</span>
                  </TableCell>
                  <TableCell>
                    <code className="bg-accent-muted px-2 py-0.5 rounded text-[11px] font-mono font-semibold tracking-wide border border-accent/30 text-accent">
                      {device.firmware}
                    </code>
                  </TableCell>
                  <TableCell>
                    <div className="w-20 h-6">
                      <ResponsiveContainer width="100%" height={24}>
                        <AreaChart data={device.activity.map((v, i) => ({ v, i }))}>
                          <Area
                            type="monotone"
                            dataKey="v"
                            stroke={getStatusColor(device.status)}
                            strokeWidth={1}
                            fill={getStatusColor(device.status)}
                            fillOpacity={0.15}
                            dot={false}
                            isAnimationActive={false}
                          />
                        </AreaChart>
                      </ResponsiveContainer>
                    </div>
                  </TableCell>
                  <TableCell>
                    <span className="font-mono text-[13px]">{device.uptime}</span>
                  </TableCell>
                  <TableCell className="w-[120px] text-right" onClick={(e) => e.stopPropagation()}>
                    <Button variant="ghost" size="sm" onClick={() => handleViewDevice(device)} title="View Details">
                      <Eye size={14} />
                    </Button>
                    <Button variant="ghost" size="sm" title="Edit Device">
                      <Pencil size={14} />
                    </Button>
                    <Button variant="ghost" size="sm" className="hover:text-danger" title="Delete Device">
                      <Trash2 size={14} />
                    </Button>
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        )}
      </Card>

      {/* Device Detail Drawer */}
      <Drawer open={isDrawerOpen} onOpenChange={setIsDrawerOpen}>
        <DrawerContent side="right" size="520px">
          {selectedDevice && (
            <>
              <DrawerHeader onClose={() => setIsDrawerOpen(false)}>
                <span className="text-sm font-semibold text-foreground">Device Details</span>
              </DrawerHeader>

              <div className="px-5 pt-4">
                <div className="flex items-center gap-3.5 p-4 bg-gradient-to-br from-surface-hover to-surface rounded-lg border border-border">
                  <span
                    className={`status-led status-led--${selectedDevice.status}`}
                    style={{ width: 10, height: 10 }}
                  />
                  <div>
                    <h4 className="font-semibold text-base tracking-tight m-0">
                      {selectedDevice.name}
                    </h4>
                    <p className="text-xs text-muted flex items-center gap-1.5 mt-0.5 m-0">
                      <span className="font-mono">{selectedDevice.id}</span>
                      <span className="opacity-30">|</span>
                      {selectedDevice.type}
                      <span className="opacity-30">|</span>
                      <span className="uppercase font-bold text-xs tracking-wider">
                        {selectedDevice.status}
                      </span>
                    </p>
                  </div>
                </div>
              </div>

              <div className="px-5">
                <Tabs value={drawerTab} onValueChange={setDrawerTab}>
                  <TabsList>
                    <TabsTrigger value="overview">Overview</TabsTrigger>
                    <TabsTrigger value="telemetry">Telemetry</TabsTrigger>
                    <TabsTrigger value="logs">Logs</TabsTrigger>
                    <TabsTrigger value="config">Config</TabsTrigger>
                  </TabsList>

                  <DrawerBody className="px-0">
                    <TabsContent value="overview">
                      {selectedDevice.status === 'offline' && (
                        <Callout
                          intent="danger"
                          icon={<AlertCircle size={16} />}
                          className="mb-4"
                        >
                          Device offline — last seen {selectedDevice.lastSeen}
                        </Callout>
                      )}
                      {selectedDevice.status === 'warning' && (
                        <Callout
                          intent="warning"
                          icon={<AlertTriangle size={16} />}
                          className="mb-4"
                        >
                          Device reporting warnings. Check telemetry.
                        </Callout>
                      )}

                      <div className="grid grid-cols-2 gap-4">
                        <div className="flex flex-col gap-1">
                          <span className="text-[10px] font-semibold uppercase tracking-wider text-muted">Device ID</span>
                          <span className="text-sm text-foreground font-mono">{selectedDevice.id}</span>
                        </div>
                        <div className="flex flex-col gap-1">
                          <span className="text-[10px] font-semibold uppercase tracking-wider text-muted">Location</span>
                          <span className="text-sm text-foreground">{selectedDevice.location}</span>
                        </div>
                        <div className="flex flex-col gap-1">
                          <span className="text-[10px] font-semibold uppercase tracking-wider text-muted">Firmware</span>
                          <span className="text-sm text-foreground font-mono">{selectedDevice.firmware}</span>
                        </div>
                        <div className="flex flex-col gap-1">
                          <span className="text-[10px] font-semibold uppercase tracking-wider text-muted">Last Seen</span>
                          <span className="text-sm text-foreground font-mono">{selectedDevice.lastSeen}</span>
                        </div>
                        <div className="flex flex-col gap-1">
                          <span className="text-[10px] font-semibold uppercase tracking-wider text-muted">Uptime</span>
                          <span className="text-sm text-foreground font-mono">{selectedDevice.uptime}</span>
                        </div>
                      </div>

                      <hr className="border-border my-4" />

                      <span className="text-[10px] font-semibold uppercase tracking-wider text-muted">Quick Actions</span>
                      <div className="grid grid-cols-2 gap-2 mt-3">
                        <Button className="justify-start font-semibold text-xs py-2.5">
                          <RefreshCw size={14} />
                          Restart
                        </Button>
                        <Button className="justify-start font-semibold text-xs py-2.5">
                          <CloudUpload size={14} />
                          Update FW
                        </Button>
                        <Button className="justify-start font-semibold text-xs py-2.5">
                          <BarChart3 size={14} />
                          Telemetry
                        </Button>
                        <Button className="justify-start font-semibold text-xs py-2.5">
                          <Settings size={14} />
                          Configure
                        </Button>
                      </div>
                    </TabsContent>

                    <TabsContent value="telemetry">
                      <div className="flex flex-col gap-5">
                        {[
                          { label: 'CPU Usage', data: telemetryData.cpu, color: '#2965CC', unit: '%' },
                          { label: 'Memory', data: telemetryData.memory, color: '#0F9960', unit: '%' },
                          { label: 'Signal Strength', data: telemetryData.signal, color: '#D99E0B', unit: 'dBm' },
                        ].map((metric) => (
                          <div
                            key={metric.label}
                            className="p-3 bg-surface-hover rounded-md border border-border"
                          >
                            <div className="flex justify-between items-center mb-2">
                              <span className="text-[10px] font-semibold uppercase tracking-wider text-muted">
                                {metric.label}
                              </span>
                              <span
                                className="font-mono text-sm"
                                style={{ color: metric.color }}
                              >
                                {metric.data[metric.data.length - 1]}{metric.unit}
                              </span>
                            </div>
                            <ResponsiveContainer width="100%" height={60}>
                              <AreaChart data={metric.data.map((v, i) => ({ v, i }))}>
                                <defs>
                                  <linearGradient id={`tel-${metric.label.replace(/\s/g, '')}`} x1="0" y1="0" x2="0" y2="1">
                                    <stop offset="0%" stopColor={metric.color} stopOpacity={0.3} />
                                    <stop offset="100%" stopColor={metric.color} stopOpacity={0} />
                                  </linearGradient>
                                </defs>
                                <Area
                                  type="monotone"
                                  dataKey="v"
                                  stroke={metric.color}
                                  strokeWidth={1.5}
                                  fill={`url(#tel-${metric.label.replace(/\s/g, '')})`}
                                  dot={false}
                                  isAnimationActive={false}
                                />
                              </AreaChart>
                            </ResponsiveContainer>
                          </div>
                        ))}
                      </div>
                    </TabsContent>

                    <TabsContent value="logs">
                      <div className="flex flex-col">
                        {logEntries.map((entry, i) => (
                          <div
                            key={i}
                            className={`flex items-baseline gap-2.5 py-1.5 text-xs ${
                              i < logEntries.length - 1 ? 'border-b border-border' : ''
                            }`}
                          >
                            <span className="text-[11px] text-muted font-mono shrink-0">
                              {entry.time}
                            </span>
                            <span
                              className={`text-[10px] font-bold font-mono px-1.5 py-px rounded shrink-0 ${
                                logLevelClasses[entry.level.toLowerCase()] ?? ''
                              }`}
                            >
                              {entry.level}
                            </span>
                            <span className="text-foreground">{entry.message}</span>
                          </div>
                        ))}
                      </div>
                    </TabsContent>

                    <TabsContent value="config">
                      <div className="flex flex-col">
                        {configEntries.map((entry, i) => (
                          <div
                            key={entry.key}
                            className={`flex items-center justify-between py-2.5 text-[13px] ${
                              i < configEntries.length - 1 ? 'border-b border-border' : ''
                            }`}
                          >
                            <span className="text-muted font-mono text-xs">{entry.key}</span>
                            <span className="text-foreground font-mono text-xs">{entry.value}</span>
                          </div>
                        ))}
                      </div>
                    </TabsContent>
                  </DrawerBody>
                </Tabs>
              </div>

              <DrawerFooter className="flex justify-between items-center bg-surface-hover">
                <Button onClick={() => setIsDrawerOpen(false)}>Close</Button>
                <Button variant="primary">
                  <Pencil size={14} />
                  Edit Device
                </Button>
              </DrawerFooter>
            </>
          )}
        </DrawerContent>
      </Drawer>
    </div>
  );
};
