import { useMemo, useState } from 'react';
import {
  Card,
  Elevation,
  H3,
  HTMLTable,
  Tag,
  Button,
  Icon,
  Callout,
  Spinner,
  Switch,
  Alert,
  Tooltip,
} from '@blueprintjs/core';
import { useRules, useDeleteRule, useToggleRule } from '../queries/use-rules';
import { useDeviceBlueprints } from '../../../hooks/use-device-blueprints';
import { useFleets } from '../../../hooks/use-fleets';
import { useAllDevices } from '../../../hooks/use-devices';
import { useUIStore } from '../../../stores/ui-store';
import { RuleDialog } from '../components/rule-dialog';
import { EmptyState, FilterPill } from '@patkepa/kantzen-ui';
import { showSuccessToast, showErrorToast } from '../../../utils/toaster';
import type { Rule } from '../../../types/rules';
import './rules.css';

export const Rules = () => {
  const [filterEnabled, setFilterEnabled] = useState<string>('all');
  const [filterTrigger, setFilterTrigger] = useState<string>('all');
  const [deleteTarget, setDeleteTarget] = useState<Rule | null>(null);

  const { openRuleDialog } = useUIStore();
  const rulesQuery = useRules();
  const deleteMutation = useDeleteRule();
  const toggleMutation = useToggleRule();
  const { data: blueprints } = useDeviceBlueprints();
  const { data: fleets } = useFleets();
  const { data: devicesData } = useAllDevices();

  const rules = useMemo(() => rulesQuery.data ?? [], [rulesQuery.data]);

  const statusCounts = useMemo(
    () => ({
      all: rules.length,
      enabled: rules.filter((r) => r.enabled).length,
      disabled: rules.filter((r) => !r.enabled).length,
    }),
    [rules],
  );

  const triggerCounts = useMemo(
    () => ({
      all: rules.length,
      telemetry: rules.filter((r) => r.trigger_type === 'telemetry').length,
      device_status: rules.filter((r) => r.trigger_type === 'device_status').length,
    }),
    [rules],
  );

  const resolveTargetName = (rule: Rule): string => {
    if (rule.target_type === 'global') return 'All devices';
    if (!rule.target_id) return rule.target_type;
    switch (rule.target_type) {
      case 'blueprint': {
        const dt = (blueprints ?? []).find((d) => d.id === rule.target_id);
        return dt ? dt.name : rule.target_id;
      }
      case 'fleet': {
        const f = (fleets ?? []).find((fl) => String(fl.id) === rule.target_id);
        return f ? f.name : rule.target_id;
      }
      case 'device': {
        const d = (devicesData?.data ?? []).find((dev) => dev.id === rule.target_id);
        return d ? d.name : rule.target_id;
      }
      default:
        return rule.target_id;
    }
  };
  const isLoading = rulesQuery.isLoading;
  const error = rulesQuery.error;

  const filteredRules = rules.filter((rule) => {
    if (filterEnabled !== 'all') {
      const wantEnabled = filterEnabled === 'enabled';
      if (rule.enabled !== wantEnabled) return false;
    }
    if (filterTrigger !== 'all' && rule.trigger_type !== filterTrigger) return false;
    return true;
  });

  const handleToggle = (rule: Rule) => {
    toggleMutation.mutate({ id: rule.id, enabled: !rule.enabled });
  };

  const handleDelete = () => {
    if (!deleteTarget) return;
    deleteMutation.mutate(deleteTarget.id, {
      onSuccess: () => {
        void showSuccessToast('Rule deleted');
        setDeleteTarget(null);
      },
      onError: () => {
        void showErrorToast('Failed to delete rule');
      },
    });
  };

  const operatorLabel: Record<string, string> = {
    gt: '>',
    gte: '>=',
    lt: '<',
    lte: '<=',
    eq: '=',
    neq: '!=',
  };

  const conditionsSummary = (rule: Rule) => {
    if (!rule.conditions || rule.conditions.length === 0) return 'No conditions';
    return rule.conditions
      .map((c) => {
        const field =
          c.selector.kind === 'metric'
            ? `${c.selector.stream_key}.${c.selector.field_path}`
            : c.selector.kind === 'geofence'
              ? `${c.selector.zone_id} ${c.selector.field}`
              : 'status';
        return `${field} ${operatorLabel[c.operator] ?? c.operator} ${c.value}`;
      })
      .join(', ');
  };

  const actionIcon = (type: string) => {
    switch (type) {
      case 'alert':
        return 'notifications';
      case 'webhook':
        return 'globe';
      case 'command':
        return 'console';
      default:
        return 'cog';
    }
  };

  const actionLabel = (action: Rule['actions'][number]) => {
    switch (action.action_type) {
      case 'alert':
        return `Alert (${(action.config.severity as string) ?? 'warning'})`;
      case 'webhook':
        return `Webhook: ${(action.config.url as string) || 'no URL'}`;
      case 'command':
        return `Command: ${(action.config.command as string) || 'no command'}`;
      default:
        return action.action_type;
    }
  };

  if (error) {
    return (
      <div className="rules-page">
        <Callout intent="danger" icon="error">
          Failed to load rules. Is the backend running?
        </Callout>
      </div>
    );
  }

  if (isLoading) {
    return (
      <div className="rules-page">
        <Spinner />
      </div>
    );
  }

  return (
    <div className="rules-page">
      {/* Header */}
      <div className="page-header">
        <div>
          <H3>Rules</H3>
          <p className="page-description">
            {filteredRules.length} of {rules.length} rules
          </p>
        </div>
        <Button intent="primary" icon="add" onClick={() => openRuleDialog()}>
          Add Rule
        </Button>
      </div>

      {/* Filters */}
      <Card elevation={Elevation.ONE} className="rules-controls">
        <div className="controls-row">
          <div className="filter-section">
            {(['all', 'enabled', 'disabled'] as const).map((status) => (
              <FilterPill
                key={status}
                value={status}
                label={status.charAt(0).toUpperCase() + status.slice(1)}
                active={filterEnabled === status}
                count={statusCounts[status]}
                onSelect={setFilterEnabled}
              />
            ))}
          </div>
          <div className="filter-section">
            {(['all', 'telemetry', 'device_status'] as const).map((trigger) => (
              <FilterPill
                key={trigger}
                value={trigger}
                label={
                  trigger === 'all'
                    ? 'All Triggers'
                    : trigger === 'telemetry'
                      ? 'Telemetry'
                      : 'Device Status'
                }
                active={filterTrigger === trigger}
                icon={
                  trigger === 'all'
                    ? undefined
                    : trigger === 'telemetry'
                      ? 'pulse'
                      : 'signal-search'
                }
                count={triggerCounts[trigger]}
                onSelect={setFilterTrigger}
              />
            ))}
          </div>
        </div>
      </Card>

      {/* Rules Table */}
      <Card elevation={Elevation.ONE} className="rules-card">
        {filteredRules.length === 0 ? (
          <EmptyState
            icon="filter"
            title="No rules found"
            description="Create a rule to start monitoring your devices"
          />
        ) : (
          <HTMLTable interactive className="rules-table">
            <thead>
              <tr>
                <th>Name</th>
                <th>Trigger</th>
                <th>Target</th>
                <th>Conditions</th>
                <th>Enabled</th>
                <th>Actions</th>
                <th style={{ width: 60 }}></th>
              </tr>
            </thead>
            <tbody>
              {filteredRules.map((rule) => (
                <tr key={rule.id} className="rule-row" onClick={() => openRuleDialog(rule.id)}>
                  <td>
                    <div className="rule-name-cell">
                      <strong>{rule.name}</strong>
                      {rule.description && (
                        <span className="rule-description">{rule.description}</span>
                      )}
                    </div>
                  </td>
                  <td>
                    <Tag minimal>
                      {rule.trigger_type === 'telemetry' ? 'Telemetry' : 'Device Status'}
                    </Tag>
                  </td>
                  <td>
                    <Tag minimal intent={rule.target_type === 'global' ? 'primary' : undefined}>
                      {rule.target_type}
                    </Tag>
                    {rule.target_type !== 'global' && (
                      <span style={{ marginLeft: 6 }}>{resolveTargetName(rule)}</span>
                    )}
                  </td>
                  <td>
                    <span className="conditions-summary">{conditionsSummary(rule)}</span>
                  </td>
                  <td onClick={(e) => e.stopPropagation()}>
                    <Switch
                      checked={rule.enabled}
                      onChange={() => handleToggle(rule)}
                      style={{ marginBottom: 0 }}
                    />
                  </td>
                  <td>
                    <div className="rule-actions-cell">
                      {(rule.actions ?? []).map((a) => (
                        <Tooltip key={a.id} content={actionLabel(a)} minimal hoverOpenDelay={150}>
                          <Icon
                            icon={actionIcon(a.action_type)}
                            size={14}
                            color="hsl(var(--muted))"
                          />
                        </Tooltip>
                      ))}
                    </div>
                  </td>
                  <td onClick={(e) => e.stopPropagation()}>
                    <Button
                      icon="trash"
                      minimal
                      small
                      intent="danger"
                      onClick={() => setDeleteTarget(rule)}
                    />
                  </td>
                </tr>
              ))}
            </tbody>
          </HTMLTable>
        )}
      </Card>

      {/* Delete Confirmation */}
      <Alert
        isOpen={deleteTarget !== null}
        onCancel={() => setDeleteTarget(null)}
        onConfirm={handleDelete}
        cancelButtonText="Cancel"
        confirmButtonText="Delete"
        intent="danger"
        icon="trash"
        loading={deleteMutation.isPending}
      >
        <p>
          Are you sure you want to delete the rule <strong>{deleteTarget?.name}</strong>? This
          action cannot be undone.
        </p>
      </Alert>

      <RuleDialog />
    </div>
  );
};
