import { useState } from 'react';
import { Button, Card, HTMLTable, NonIdealState, Tag } from '@blueprintjs/core';
import { useApiKeys, useDeleteApiKey } from '../../hooks/use-api-keys';
import { CreateApiKeyDialog } from '../../components/settings/create-api-key-dialog';

export const ApiKeysSettings = () => {
  const [dialogOpen, setDialogOpen] = useState(false);
  const { data: keys, isLoading, error } = useApiKeys();
  const deleteMutation = useDeleteApiKey();

  if (error) {
    return <NonIdealState icon="error" title="Failed to load API keys" />;
  }

  return (
    <div>
      <div
        style={{
          display: 'flex',
          justifyContent: 'space-between',
          alignItems: 'center',
          marginBottom: 16,
        }}
      >
        <h2 style={{ margin: 0 }}>API Keys</h2>
        <Button icon="plus" intent="primary" onClick={() => setDialogOpen(true)}>
          Create API Key
        </Button>
      </div>

      <Card>
        {isLoading ? (
          <NonIdealState icon="time" title="Loading..." />
        ) : !keys?.length ? (
          <NonIdealState
            icon="key"
            title="No API keys"
            description="Create an API key to allow CI pipelines to register firmware."
          />
        ) : (
          <HTMLTable striped style={{ width: '100%' }}>
            <thead>
              <tr>
                <th>Name</th>
                <th>Key Prefix</th>
                <th>Scope</th>
                <th>Last Used</th>
                <th>Created</th>
                <th />
              </tr>
            </thead>
            <tbody>
              {keys.map((key) => (
                <tr key={key.id}>
                  <td>{key.name}</td>
                  <td>
                    <code>{key.key_prefix}...</code>
                  </td>
                  <td>
                    <Tag minimal>{key.device_type_name ?? 'All'}</Tag>
                  </td>
                  <td>{key.last_used_at ?? 'Never'}</td>
                  <td>{key.created_at}</td>
                  <td>
                    <Button
                      icon="trash"
                      intent="danger"
                      minimal
                      small
                      loading={deleteMutation.isPending}
                      onClick={() => {
                        if (confirm('Revoke this API key? This cannot be undone.')) {
                          deleteMutation.mutate(key.id);
                        }
                      }}
                    />
                  </td>
                </tr>
              ))}
            </tbody>
          </HTMLTable>
        )}
      </Card>

      <CreateApiKeyDialog isOpen={dialogOpen} onClose={() => setDialogOpen(false)} />
    </div>
  );
};
