import type { DeviceBlueprintRevision, DeviceContract } from '../../../types/api';

export interface RuleMetricFieldOption {
  value: string;
  label: string;
  route?: string;
  blueprint_id?: string;
  blueprint_revision_id?: string;
}

interface BlueprintField {
  path?: string;
  type?: string;
  label?: string;
  semantic?: string;
}

interface BlueprintStream {
  key?: string;
  route?: string;
  fields?: BlueprintField[];
}

interface BlueprintDocument {
  spec?: {
    streams?: BlueprintStream[];
    commands?: Array<{ key?: string; label?: string }>;
  };
}

interface ContractField {
  valueType?: string;
  label?: string;
  semantic?: string;
}

interface ContractStream {
  route?: string;
  fields?: Record<string, ContractField>;
}

interface ContractDocument {
  streams?: Record<string, ContractStream>;
  commands?: Record<string, { label?: string }>;
}

export function blueprintRuleCommands(
  revision: DeviceBlueprintRevision | undefined,
): RuleMetricFieldOption[] {
  const document = revision?.document as BlueprintDocument | undefined;
  return (document?.spec?.commands ?? []).flatMap((command) =>
    command.key ? [{ value: command.key, label: command.label ?? command.key }] : [],
  );
}

export function contractRuleCommands(
  contract: DeviceContract | undefined,
): RuleMetricFieldOption[] {
  const document = contract?.document as ContractDocument | undefined;
  return Object.entries(document?.commands ?? {}).map(([key, command]) => ({
    value: key,
    label: command.label ?? key,
  }));
}

function isNumeric(valueType: string | undefined): boolean {
  return (
    valueType === 'float64' ||
    valueType === 'int64' ||
    valueType === 'number' ||
    valueType === 'integer'
  );
}

function canonicalMetric(streamKey: string, path: string): string {
  return `${streamKey}.${path}`;
}

function option(
  streamKey: string,
  path: string,
  label: string | undefined,
  semantic: string | undefined,
  route?: string,
  blueprintId?: string,
  revisionId?: string,
): RuleMetricFieldOption {
  const value = canonicalMetric(streamKey, path);
  const displayLabel = label ?? value;
  return {
    value,
    label: semantic ? `${displayLabel} · ${semantic}` : displayLabel,
    ...(route && { route }),
    blueprint_id: blueprintId,
    blueprint_revision_id: revisionId,
  };
}

export function blueprintRuleMetricFields(
  revision: DeviceBlueprintRevision | undefined,
): RuleMetricFieldOption[] {
  const document = revision?.document as BlueprintDocument | undefined;
  return (document?.spec?.streams ?? []).flatMap((stream) => {
    if (!stream.key) return [];
    return (stream.fields ?? [])
      .filter((field) => field.path && isNumeric(field.type))
      .map((field) =>
        option(
          stream.key!,
          field.path!,
          field.label,
          field.semantic,
          stream.route,
          revision?.blueprint_id,
          revision?.id,
        ),
      );
  });
}

export function contractRuleMetricFields(
  contract: DeviceContract | undefined,
  blueprintId?: string,
): RuleMetricFieldOption[] {
  const document = contract?.document as ContractDocument | undefined;
  return Object.entries(document?.streams ?? {}).flatMap(([streamKey, stream]) =>
    Object.entries(stream.fields ?? {})
      .filter(([, field]) => isNumeric(field.valueType))
      .map(([path, field]) =>
        option(
          streamKey,
          path,
          field.label,
          field.semantic,
          stream.route,
          blueprintId,
          contract?.blueprint_revision_id,
        ),
      ),
  );
}
