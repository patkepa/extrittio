export interface Alert {
  id: string;
  rule_id: string | null;
  device_id: string;
  severity: 'info' | 'warning' | 'critical';
  status: 'active' | 'acknowledged' | 'resolved';
  message: string;
  triggered_value: string | null;
  resolved_at: string | null;
  acknowledged_at: string | null;
  created_at: string;
}

export interface AlertSummary {
  active: SeverityCounts;
  acknowledged: SeverityCounts;
  total_active: number;
}

export interface SeverityCounts {
  info: number;
  warning: number;
  critical: number;
}
