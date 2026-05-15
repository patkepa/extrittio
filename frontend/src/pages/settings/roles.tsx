import { useState } from 'react';
import {
  Button,
  Callout,
  Card,
  Checkbox,
  Dialog,
  DialogBody,
  DialogFooter,
  Elevation,
  FormGroup,
  H3,
  H4,
  HTMLTable,
  Icon,
  InputGroup,
  Spinner,
  Tag,
  TextArea,
} from '@blueprintjs/core';
import { showErrorToast, showSuccessToast } from '../../utils/toaster';
import {
  useCreateRole,
  useDeleteRole,
  usePermissions,
  useRoles,
  useUpdateRole,
} from '../../hooks/use-roles';
import { hasPermission } from '../../auth/permissions';
import { useAuthStore } from '../../stores/auth-store';
import type { Role } from '../../types/api';
import './settings.css';

export const RolesSettings = () => {
  const permissions = useAuthStore((s) => s.user?.permissions);
  const canManageRoles = hasPermission(permissions, 'roles.manage');
  const { data: roles = [], isLoading, error } = useRoles();
  const { data: availablePermissions = [] } = usePermissions();
  const createRoleMutation = useCreateRole();
  const updateRoleMutation = useUpdateRole();
  const deleteRoleMutation = useDeleteRole();
  const [editingRole, setEditingRole] = useState<Role | null>(null);
  const [isDialogOpen, setIsDialogOpen] = useState(false);
  const [name, setName] = useState('');
  const [description, setDescription] = useState('');
  const [selectedPermissions, setSelectedPermissions] = useState<string[]>([]);

  const resetDialog = () => {
    setEditingRole(null);
    setName('');
    setDescription('');
    setSelectedPermissions([]);
    setIsDialogOpen(false);
  };

  const openCreateDialog = () => {
    setEditingRole(null);
    setName('');
    setDescription('');
    setSelectedPermissions([]);
    setIsDialogOpen(true);
  };

  const openEditDialog = (role: Role) => {
    setEditingRole(role);
    setName(role.name);
    setDescription(role.description ?? '');
    setSelectedPermissions(role.permissions);
    setIsDialogOpen(true);
  };

  const togglePermission = (key: string, checked: boolean) => {
    if (checked) {
      setSelectedPermissions((current) => [...current, key]);
    } else {
      setSelectedPermissions((current) => current.filter((permission) => permission !== key));
    }
  };

  const handleSubmit = () => {
    const body = {
      name,
      description: description.trim() ? description.trim() : null,
      permissions: selectedPermissions,
    };

    if (editingRole) {
      updateRoleMutation.mutate(
        { id: editingRole.id, body },
        {
          onSuccess: () => {
            resetDialog();
            void showSuccessToast('Role updated');
          },
          onError: () => void showErrorToast('Failed to update role'),
        },
      );
    } else {
      createRoleMutation.mutate(body, {
        onSuccess: () => {
          resetDialog();
          void showSuccessToast('Role created');
        },
        onError: () => void showErrorToast('Failed to create role'),
      });
    }
  };

  if (error) {
    return (
      <div className="settings-page">
        <Callout intent="danger" icon="error">
          Failed to load roles.
        </Callout>
      </div>
    );
  }

  if (isLoading) {
    return (
      <div className="settings-page">
        <Spinner />
      </div>
    );
  }

  return (
    <div className="settings-page settings-page-wide">
      <div className="page-header">
        <div>
          <H3>Roles</H3>
          <p className="page-description">
            {roles.length} role{roles.length !== 1 ? 's' : ''}
          </p>
        </div>
        {canManageRoles && (
          <Button intent="primary" icon="add" onClick={openCreateDialog}>
            Add Role
          </Button>
        )}
      </div>

      <Card elevation={Elevation.ONE} className="settings-table-card">
        {roles.length === 0 ? (
          <div className="settings-empty">
            <Icon icon="shield" size={48} />
            <H4>No roles found</H4>
            <p>Create roles to control user permissions</p>
          </div>
        ) : (
          <HTMLTable interactive className="settings-table">
            <thead>
              <tr>
                <th>Name</th>
                <th>Permissions</th>
                <th>Users</th>
                {canManageRoles && <th className="actions-column">Actions</th>}
              </tr>
            </thead>
            <tbody>
              {roles.map((role) => (
                <tr key={role.id}>
                  <td>
                    <div className="role-name-cell">
                      <strong>{role.name}</strong>
                      {role.is_system && <Tag minimal>Built-in</Tag>}
                    </div>
                    {role.description && <p className="role-description">{role.description}</p>}
                  </td>
                  <td>{role.permissions.length}</td>
                  <td>{role.user_count}</td>
                  {canManageRoles && (
                    <td className="actions-column">
                      <Button
                        icon="edit"
                        minimal
                        small
                        disabled={role.is_system}
                        onClick={() => openEditDialog(role)}
                      />
                      <Button
                        icon="trash"
                        minimal
                        small
                        intent="danger"
                        disabled={role.is_system || role.user_count > 0}
                        loading={
                          deleteRoleMutation.isPending && deleteRoleMutation.variables === role.id
                        }
                        onClick={() =>
                          deleteRoleMutation.mutate(role.id, {
                            onSuccess: () => void showSuccessToast('Role deleted'),
                            onError: () => void showErrorToast('Failed to delete role'),
                          })
                        }
                      />
                    </td>
                  )}
                </tr>
              ))}
            </tbody>
          </HTMLTable>
        )}
      </Card>

      <Dialog
        icon="shield"
        title={editingRole ? 'Edit Role' : 'Add Role'}
        isOpen={isDialogOpen}
        onClose={resetDialog}
      >
        <DialogBody>
          <FormGroup label="Name" labelInfo="(required)">
            <InputGroup
              placeholder="e.g. firmware_manager"
              value={name}
              disabled={!!editingRole}
              onChange={(event) => setName(event.target.value)}
            />
          </FormGroup>
          <FormGroup label="Description">
            <TextArea
              fill
              value={description}
              onChange={(event) => setDescription(event.target.value)}
            />
          </FormGroup>
          <FormGroup label="Permissions">
            <div className="permission-checkbox-list">
              {availablePermissions.map((permission) => (
                <Checkbox
                  key={permission.key}
                  checked={selectedPermissions.includes(permission.key)}
                  label={permissionLabel(permission.key)}
                  onChange={(event) => togglePermission(permission.key, event.currentTarget.checked)}
                />
              ))}
            </div>
          </FormGroup>
        </DialogBody>
        <DialogFooter
          actions={
            <>
              <Button onClick={resetDialog}>Cancel</Button>
              <Button
                intent="primary"
                icon="tick"
                onClick={handleSubmit}
                loading={createRoleMutation.isPending || updateRoleMutation.isPending}
                disabled={!name.trim()}
              >
                {editingRole ? 'Save Role' : 'Add Role'}
              </Button>
            </>
          }
        />
      </Dialog>
    </div>
  );
};

function permissionLabel(key: string): string {
  return key
    .split('.')
    .map((part) => part.replaceAll('_', ' '))
    .join(' / ');
}
