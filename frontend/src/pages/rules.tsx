import { useState } from 'react';
import {
  Card,
  Elevation,
  H3,
  HTMLTable,
  Tag,
  Button,
  HTMLSelect,
  Icon,
  H4,
  Callout,
  Spinner,
  Switch,
  Alert,
} from '@blueprintjs/core';
import { useRules, useDeleteRule, useToggleRule } from '../hooks/use-rules';
import { useUIStore } from '../stores/ui-store';
import { RuleDialog } from '../components/rules/rule-dialog';
import { showSuccessToast, showErrorToast } from '../utils/toaster';
import type { Rule } from '../types/rules';
import './rules.css';

export const Rules = () => {
  const [filterEnabled, setFilterEnabled] = useState<string>('all');
  const [filterTrigger, setFilterTrigger] = useState<string>('all');
  const [deleteTarget, setDeleteTarget] = useState<Rule | null>(null);

  const { openRuleDialog } = useUIStore();
  const rulesQuery = useRules();
  const deleteMutation = useDeleteRule();
  const toggleMutation = useToggleRule();

  const rules = rulesQuery.data ?? [];
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

  const handleToggle = (rule: Rule, e: React.MouseEvent) => {
    e.stopPropagation();
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

  const conditionsSummary = (rule: Rule) => {
    if (rule.conditions.length === 0) return 'No conditions';
    return rule.conditions.map((c) => `${c.field} ${c.operator} ${c.value}`).join(', ');
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
            <HTMLSelect value={filterEnabled} onChange={(e) => setFilterEnabled(e.target.value)}>
              <option value="all">All Status</option>
              <option value="enabled">Enabled</option>
              <option value="disabled">Disabled</option>
            </HTMLSelect>
            <HTMLSelect value={filterTrigger} onChange={(e) => setFilterTrigger(e.target.value)}>
              <option value="all">All Triggers</option>
              <option value="telemetry">Telemetry</option>
              <option value="device_status">Device Status</option>
            </HTMLSelect>
          </div>
        </div>
      </Card>

      {/* Rules Table */}
      <Card elevation={Elevation.ONE} className="rules-card">
        {filteredRules.length === 0 ? (
          <div className="empty-state">
            <Icon icon="filter" size={48} />
            <H4>No rules found</H4>
            <p>Create a rule to start monitoring your devices</p>
          </div>
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
                <tr
                  key={rule.id}
                  className="rule-row"
                  onClick={() => openRuleDialog(rule.id)}
                >
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
                    {rule.target_id && (
                      <span
                        className="mono-data"
                        style={{ fontSize: 10, marginLeft: 6, color: 'hsl(var(--muted))' }}
                      >
                        {rule.target_id}
                      </span>
                    )}
                  </td>
                  <td>
                    <span className="conditions-summary">{conditionsSummary(rule)}</span>
                  </td>
                  <td onClick={(e) => e.stopPropagation()}>
                    <Switch
                      checked={rule.enabled}
                      onChange={(e) =>
                        handleToggle(rule, e as unknown as React.MouseEvent)
                      }
                      style={{ marginBottom: 0 }}
                    />
                  </td>
                  <td>
                    <div className="rule-actions-cell">
                      {rule.actions.map((a) => (
                        <Icon
                          key={a.id}
                          icon={actionIcon(a.action_type)}
                          size={14}
                          color="hsl(var(--muted))"
                          title={a.action_type}
                        />
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
