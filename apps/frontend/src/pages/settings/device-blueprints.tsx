import { useState } from 'react';
import {
  Button,
  Callout,
  Card,
  Dialog,
  DialogBody,
  DialogFooter,
  Elevation,
  H3,
  H4,
  Spinner,
  Tag,
  TextArea,
} from '@blueprintjs/core';
import { hasPermission } from '../../auth/permissions';
import {
  useCreateDeviceBlueprint,
  useDeviceBlueprintDraft,
  useDeviceBlueprints,
  usePublishDeviceBlueprintDraft,
  useReplaceDeviceBlueprintDraft,
  useValidateDeviceBlueprintDraft,
} from '../../hooks/use-device-blueprints';
import { useAuthStore } from '../../stores/auth-store';
import type {
  DeviceBlueprint,
  DeviceBlueprintDocument,
  DeviceBlueprintValidation,
} from '../../types/api';
import { showErrorToast, showSuccessToast } from '../../utils/toaster';
import starterBlueprint from './starter-device-blueprint.json';
import './settings.css';

const STARTER_BLUEPRINT: DeviceBlueprintDocument = starterBlueprint;

const STARTER_BLUEPRINT_TEXT = JSON.stringify(STARTER_BLUEPRINT, null, 2);

function parseDocument(source: string): DeviceBlueprintDocument | null {
  try {
    const value: unknown = JSON.parse(source);
    if (typeof value !== 'object' || value === null || Array.isArray(value)) return null;
    return value as DeviceBlueprintDocument;
  } catch {
    return null;
  }
}

function ValidationResult({ result }: { result: DeviceBlueprintValidation | null }) {
  if (!result) return null;
  if (result.valid) {
    return (
      <Callout intent="success" icon="tick" compact>
        This draft is valid and ready to publish.
      </Callout>
    );
  }
  return (
    <Callout intent="danger" icon="error" title="Blueprint validation failed">
      <ul className="blueprint-validation-list">
        {result.issues.map((issue) => (
          <li key={`${issue.path}:${issue.message}`}>
            <code>{issue.path}</code> — {issue.message}
          </li>
        ))}
      </ul>
    </Callout>
  );
}

function BlueprintEditor({
  blueprint,
  canManage,
}: {
  blueprint: DeviceBlueprint;
  canManage: boolean;
}) {
  const draftQuery = useDeviceBlueprintDraft(blueprint.id);
  if (draftQuery.isLoading) return <Spinner />;
  if (draftQuery.isError || !draftQuery.data) {
    return <Callout intent="danger">Failed to load the blueprint draft.</Callout>;
  }
  return (
    <LoadedBlueprintEditor
      key={`${draftQuery.data.id}:${draftQuery.data.updated_at}`}
      blueprint={blueprint}
      document={draftQuery.data.document as unknown as DeviceBlueprintDocument}
      canManage={canManage}
    />
  );
}

function LoadedBlueprintEditor({
  blueprint,
  document,
  canManage,
}: {
  blueprint: DeviceBlueprint;
  document: DeviceBlueprintDocument;
  canManage: boolean;
}) {
  const [source, setSource] = useState(() => JSON.stringify(document, null, 2));
  const [validation, setValidation] = useState<DeviceBlueprintValidation | null>(null);
  const replaceMutation = useReplaceDeviceBlueprintDraft();
  const validateMutation = useValidateDeviceBlueprintDraft();
  const publishMutation = usePublishDeviceBlueprintDraft();
  const parsed = parseDocument(source);
  const isPending =
    replaceMutation.isPending || validateMutation.isPending || publishMutation.isPending;

  const save = async () => {
    if (!parsed) return false;
    await replaceMutation.mutateAsync({ blueprintId: blueprint.id, document: parsed });
    setValidation(null);
    return true;
  };

  const handleSave = async () => {
    try {
      if (await save()) void showSuccessToast('Blueprint draft saved');
    } catch {
      void showErrorToast('Failed to save blueprint draft');
    }
  };

  const handleValidate = async () => {
    try {
      if (!(await save())) return;
      const result = await validateMutation.mutateAsync(blueprint.id);
      setValidation(result);
    } catch {
      void showErrorToast('Failed to validate blueprint draft');
    }
  };

  const handlePublish = async () => {
    try {
      if (!(await save())) return;
      const result = await validateMutation.mutateAsync(blueprint.id);
      setValidation(result);
      if (!result.valid) return;
      const revision = await publishMutation.mutateAsync(blueprint.id);
      void showSuccessToast(`Published blueprint revision ${revision.revision}`);
    } catch {
      void showErrorToast('Failed to publish blueprint draft');
    }
  };

  return (
    <Card elevation={Elevation.ONE} className="settings-card blueprint-editor-card">
      <div className="blueprint-editor-header">
        <div>
          <H4>{blueprint.name}</H4>
          <span className="mono-data">{blueprint.key}</span>
        </div>
        <div className="blueprint-editor-actions">
          <Button disabled={!canManage || !parsed || isPending} onClick={() => void handleSave()}>
            Save draft
          </Button>
          <Button
            disabled={!canManage || !parsed || isPending}
            onClick={() => void handleValidate()}
          >
            Validate
          </Button>
          <Button
            intent="primary"
            icon="endorsed"
            disabled={!canManage || !parsed || isPending}
            loading={publishMutation.isPending}
            onClick={() => void handlePublish()}
          >
            Publish revision
          </Button>
        </div>
      </div>
      {!parsed ? (
        <Callout intent="danger" icon="error" compact>
          The editor must contain one valid JSON object.
        </Callout>
      ) : null}
      <TextArea
        className="blueprint-json-editor mono-data"
        fill
        value={source}
        disabled={!canManage}
        spellCheck={false}
        aria-label={`${blueprint.name} blueprint JSON`}
        onChange={(event) => {
          setSource(event.target.value);
          setValidation(null);
        }}
      />
      <ValidationResult result={validation} />
    </Card>
  );
}

export function DeviceBlueprintsSettings() {
  const permissions = useAuthStore((state) => state.user?.permissions);
  const canManage = hasPermission(permissions, 'device_blueprints.manage');
  const blueprintsQuery = useDeviceBlueprints();
  const createMutation = useCreateDeviceBlueprint();
  const [selectedId, setSelectedId] = useState('');
  const [isCreateOpen, setIsCreateOpen] = useState(false);
  const [newSource, setNewSource] = useState(STARTER_BLUEPRINT_TEXT);
  const blueprints = blueprintsQuery.data ?? [];
  const activeId = selectedId || blueprints[0]?.id || '';
  const activeBlueprint = blueprints.find((blueprint) => blueprint.id === activeId);
  const newDocument = parseDocument(newSource);

  const handleCreate = async () => {
    if (!newDocument) return;
    try {
      const blueprint = await createMutation.mutateAsync(newDocument);
      setSelectedId(blueprint.id);
      setIsCreateOpen(false);
      setNewSource(STARTER_BLUEPRINT_TEXT);
      void showSuccessToast('Device blueprint created');
    } catch {
      void showErrorToast('Failed to create device blueprint');
    }
  };

  if (blueprintsQuery.isLoading) return <Spinner />;
  if (blueprintsQuery.isError) {
    return <Callout intent="danger">Failed to load device blueprints.</Callout>;
  }

  return (
    <div className="settings-page settings-page-wide">
      <div className="page-header">
        <div>
          <H3>Device Blueprints</H3>
          <p className="page-description">
            Define the complete device contract: transport, routes, schemas, telemetry, commands,
            configuration, health, firmware, and presentation.
          </p>
        </div>
        <Button
          intent="primary"
          icon="add"
          disabled={!canManage}
          onClick={() => setIsCreateOpen(true)}
        >
          New blueprint
        </Button>
      </div>

      <div className="blueprint-workspace">
        <Card elevation={Elevation.ONE} className="blueprint-list-card">
          {blueprints.length === 0 ? (
            <div className="settings-empty">
              <H4>No blueprints yet</H4>
              <span>Create one to define your first device contract.</span>
            </div>
          ) : (
            blueprints.map((blueprint) => (
              <button
                type="button"
                key={blueprint.id}
                className={`blueprint-list-item ${blueprint.id === activeId ? 'active' : ''}`}
                onClick={() => setSelectedId(blueprint.id)}
              >
                <span>
                  <strong>{blueprint.name}</strong>
                  <small className="mono-data">{blueprint.key}</small>
                </span>
                {blueprint.latest_revision == null ? (
                  <Tag minimal>Draft</Tag>
                ) : (
                  <Tag intent="success" minimal>
                    r{blueprint.latest_revision}
                  </Tag>
                )}
              </button>
            ))
          )}
        </Card>
        <div className="blueprint-editor-column">
          {activeBlueprint ? (
            <BlueprintEditor blueprint={activeBlueprint} canManage={canManage} />
          ) : (
            <Callout icon="info-sign">Create a blueprint to begin.</Callout>
          )}
        </div>
      </div>

      <Dialog
        title="Create device blueprint"
        icon="diagram-tree"
        isOpen={isCreateOpen}
        onClose={() => setIsCreateOpen(false)}
        className="blueprint-create-dialog"
      >
        <DialogBody>
          <p className="page-description">
            Start with a valid temperature-sensor contract and edit its JSON before creating it.
          </p>
          {!newDocument ? (
            <Callout intent="danger" compact>
              The editor must contain one valid JSON object.
            </Callout>
          ) : null}
          <TextArea
            className="blueprint-json-editor mono-data"
            fill
            value={newSource}
            spellCheck={false}
            aria-label="New blueprint JSON"
            onChange={(event) => setNewSource(event.target.value)}
          />
        </DialogBody>
        <DialogFooter
          actions={
            <>
              <Button onClick={() => setIsCreateOpen(false)}>Cancel</Button>
              <Button
                intent="primary"
                disabled={!newDocument}
                loading={createMutation.isPending}
                onClick={() => void handleCreate()}
              >
                Create draft
              </Button>
            </>
          }
        />
      </Dialog>
    </div>
  );
}
