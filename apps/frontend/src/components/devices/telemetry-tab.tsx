import { Callout, Spinner } from '@blueprintjs/core';
import { useDeviceContract } from '../../hooks/use-devices';
import { ContractTelemetryTab } from './contract-telemetry-tab';

export const TelemetryTab = ({ deviceId }: { deviceId: string }) => {
  const contractQuery = useDeviceContract(deviceId, { retry: false });
  if (contractQuery.isLoading) return <Spinner />;
  if (contractQuery.isError || !contractQuery.data) {
    return (
      <Callout intent="danger" icon="error">
        Unable to load the assigned device contract. Telemetry requires a published blueprint and an
        assigned contract.
      </Callout>
    );
  }
  return <ContractTelemetryTab deviceId={deviceId} contract={contractQuery.data} />;
};
