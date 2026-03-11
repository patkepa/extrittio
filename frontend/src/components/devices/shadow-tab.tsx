import { useEffect, useState } from 'react';
import { Button, Callout, Divider, Spinner, Tag } from '@blueprintjs/core';
import { useDeviceShadow, useUpdateDesiredState, useDeleteDeviceShadow } from '../../hooks/use-shadow';
import { showSuccessToast, showErrorToast } from '../../utils/toaster';

interface ShadowTabProps {
  deviceId: string;
}

export const ShadowTab = ({ deviceId }: ShadowTabProps) => {
  const { data: shadow, isLoading, isError } = useDeviceShadow(deviceId);
  const updateDesiredMutation = useUpdateDesiredState();
  const deleteShadowMutation = useDeleteDeviceShadow();
  const [isEditing, setIsEditing] = useState(false);
  const [editValue, setEditValue] = useState('');
  const [jsonError, setJsonError] = useState<string | null>(null);

  useEffect(() => {
    if (!isEditing && shadow) {
      setEditValue(JSON.stringify(shadow.desired, null, 2));
    }
  }, [shadow, isEditing]);

  if (isLoading) return <Spinner />;

  if (isError) {
    return (
      <Callout intent="danger" icon="error">
        Failed to load device shadow. Try refreshing the page.
      </Callout>
    );
  }

  if (!shadow) {
    return <Callout icon="info-sign">No shadow data available.</Callout>;
  }

  const hasDelta = Object.keys(shadow.delta).length > 0;

  const handleEdit = () => {
    setEditValue(JSON.stringify(shadow.desired, null, 2));
    setJsonError(null);
    setIsEditing(true);
  };

  const handleCancel = () => {
    setIsEditing(false);
    setJsonError(null);
  };

  const handleChange = (value: string) => {
    setEditValue(value);
    try {
      JSON.parse(value);
      setJsonError(null);
    } catch (e) {
      setJsonError((e as SyntaxError).message);
    }
  };

  const handleSave = () => {
    try {
      const parsed = JSON.parse(editValue);
      updateDesiredMutation.mutate(
        { deviceId, state: parsed },
        {
          onSuccess: () => {
            setIsEditing(false);
            setJsonError(null);
            void showSuccessToast('Desired state updated');
          },
          onError: () => void showErrorToast('Failed to update desired state'),
        }
      );
    } catch (e) {
      setJsonError((e as SyntaxError).message);
    }
  };

  return (
    <div className="shadow-tab">
      <div className="shadow-status-row">
        <Tag intent={hasDelta ? 'warning' : 'success'} minimal large>
          {hasDelta ? 'Pending' : 'In Sync'}
        </Tag>
        <span className="mono-data" style={{ fontSize: 12, opacity: 0.6 }}>
          v{shadow.version}
        </span>
      </div>

      <div className="shadow-panes-horizontal">
        <div className="shadow-pane">
          <div className="shadow-pane-header">
            <span className="section-label">Desired State</span>
            {!isEditing && (
              <Button icon="edit" minimal small onClick={handleEdit}>
                Edit
              </Button>
            )}
          </div>
          {isEditing ? (
            <>
              <textarea
                className="shadow-json-editor mono-data"
                value={editValue}
                onChange={(e) => handleChange(e.target.value)}
                spellCheck={false}
              />
              {jsonError && (
                <div className="shadow-json-error">{jsonError}</div>
              )}
              <div className="shadow-edit-actions">
                <Button
                  intent="primary"
                  icon="floppy-disk"
                  small
                  loading={updateDesiredMutation.isPending}
                  disabled={!!jsonError}
                  onClick={handleSave}
                >
                  Save
                </Button>
                <Button small minimal onClick={handleCancel}>
                  Cancel
                </Button>
              </div>
            </>
          ) : (
            <pre className="shadow-json mono-data">
              {JSON.stringify(shadow.desired, null, 2)}
            </pre>
          )}
        </div>
        <div className="shadow-pane">
          <span className="section-label">Reported State</span>
          <pre className="shadow-json mono-data">
            {JSON.stringify(shadow.reported, null, 2)}
          </pre>
        </div>
      </div>

      {hasDelta && (
        <div className="shadow-pane shadow-delta-pane" style={{ marginTop: 12 }}>
          <span className="section-label">Delta</span>
          <pre className="shadow-diff mono-data">
            {Object.entries(shadow.delta).map(([key, desiredVal]) => {
              const reportedVal = (shadow.reported as Record<string, unknown>)[key];
              const lines: React.ReactNode[] = [];
              if (reportedVal !== undefined) {
                lines.push(
                  <span key={`${key}-old`} className="diff-line diff-removed">
                    {`- "${key}": ${JSON.stringify(reportedVal)}`}
                  </span>
                );
              }
              lines.push(
                <span key={`${key}-new`} className="diff-line diff-added">
                  {`+ "${key}": ${JSON.stringify(desiredVal)}`}
                </span>
              );
              return lines;
            })}
          </pre>
        </div>
      )}

      <Divider className="tab-divider" />

      <div className="shadow-actions tab-actions">
        <Button
          intent="danger"
          icon="trash"
          minimal
          loading={deleteShadowMutation.isPending}
          onClick={() =>
            deleteShadowMutation.mutate(deviceId, {
              onSuccess: () => void showSuccessToast('Shadow cleared'),
              onError: () => void showErrorToast('Failed to clear shadow'),
            })
          }
        >
          Clear Shadow
        </Button>
      </div>
    </div>
  );
};
