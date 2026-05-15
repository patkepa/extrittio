import { useState } from 'react';
import { showSuccessToast, showErrorToast } from '../../utils/toaster';
import {
  Card,
  Elevation,
  H3,
  H4,
  HTMLTable,
  Button,
  Dialog,
  DialogBody,
  DialogFooter,
  FormGroup,
  InputGroup,
  Callout,
  Spinner,
  Icon,
  Checkbox,
  Tag,
} from '@blueprintjs/core';
import { useConfirmShortcut } from '@extrittio/interactions';
import { useRoles } from '../../hooks/use-roles';
import { useUsers, useCreateUser, useDeleteUser, useSetUserRoles } from '../../hooks/use-users';
import type { AuthUser, Role } from '../../types/api';
import './settings.css';

export const UsersSettings = () => {
  const { data: users = [], isLoading, error } = useUsers();
  const { data: roles = [] } = useRoles();
  const createUserMutation = useCreateUser();
  const deleteUserMutation = useDeleteUser();
  const setUserRolesMutation = useSetUserRoles();
  const [isDialogOpen, setIsDialogOpen] = useState(false);
  const [editingUser, setEditingUser] = useState<AuthUser | null>(null);
  const [newUsername, setNewUsername] = useState('');
  const [newPassword, setNewPassword] = useState('');
  const [selectedRoleIds, setSelectedRoleIds] = useState<number[]>([]);
  const [editingRoleIds, setEditingRoleIds] = useState<number[]>([]);

  const defaultRoleIds = () => {
    const viewer = roles.find((role) => role.name === 'viewer');
    return viewer ? [viewer.id] : roles.slice(0, 1).map((role) => role.id);
  };

  const openCreateDialog = () => {
    setSelectedRoleIds(defaultRoleIds());
    setIsDialogOpen(true);
  };

  const handleCreate = () => {
    createUserMutation.mutate(
      { username: newUsername, password: newPassword, role_ids: selectedRoleIds },
      {
        onSuccess: () => {
          setIsDialogOpen(false);
          setNewUsername('');
          setNewPassword('');
          void showSuccessToast('User created');
        },
        onError: () => {
          void showErrorToast('Failed to create user');
        },
      },
    );
  };

  const canCreateUser =
    !!newUsername.trim() &&
    !!newPassword.trim() &&
    selectedRoleIds.length > 0 &&
    !createUserMutation.isPending;

  const openEditRoles = (user: AuthUser) => {
    setEditingUser(user);
    setEditingRoleIds(user.roles?.map((role) => role.id) ?? []);
  };

  const handleUpdateRoles = () => {
    if (!editingUser) return;

    setUserRolesMutation.mutate(
      { id: editingUser.id, roleIds: editingRoleIds },
      {
        onSuccess: () => {
          setEditingUser(null);
          setEditingRoleIds([]);
          void showSuccessToast('Roles updated');
        },
        onError: () => {
          void showErrorToast('Failed to update roles');
        },
      },
    );
  };

  useConfirmShortcut({
    isOpen: isDialogOpen,
    canConfirm: canCreateUser,
    onConfirm: handleCreate,
  });

  if (error) {
    return (
      <div className="settings-page">
        <Callout intent="danger" icon="error">
          Failed to load users. Is the backend running?
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
    <div className="settings-page">
      <div className="page-header">
        <div>
          <H3>Users</H3>
          <p className="page-description">
            {users.length} user{users.length !== 1 ? 's' : ''}
          </p>
        </div>
        <Button intent="primary" icon="add" onClick={openCreateDialog}>
          Add User
        </Button>
      </div>

      <Card elevation={Elevation.ONE} className="settings-table-card">
        {users.length === 0 ? (
          <div className="settings-empty">
            <Icon icon="people" size={48} />
            <H4>No users found</H4>
            <p>Add users to grant access to the platform</p>
          </div>
        ) : (
          <HTMLTable interactive className="settings-table">
            <thead>
              <tr>
                <th>Username</th>
                <th>Role</th>
                <th className="actions-column">Actions</th>
              </tr>
            </thead>
            <tbody>
              {users.map((user) => (
                <tr key={user.id}>
                  <td>
                    <strong>{user.username}</strong>
                  </td>
                  <td>
                    <RoleTags user={user} />
                  </td>
                  <td className="actions-column">
                    <Button
                      icon="shield"
                      minimal
                      small
                      disabled={roles.length === 0}
                      onClick={() => openEditRoles(user)}
                    />
                    <Button
                      icon="trash"
                      minimal
                      small
                      intent="danger"
                      disabled={users.length <= 1}
                      loading={
                        deleteUserMutation.isPending && deleteUserMutation.variables === user.id
                      }
                      onClick={() =>
                        deleteUserMutation.mutate(user.id, {
                          onSuccess: () => void showSuccessToast('User deleted'),
                          onError: () => void showErrorToast('Failed to delete user'),
                        })
                      }
                    />
                  </td>
                </tr>
              ))}
            </tbody>
          </HTMLTable>
        )}
      </Card>

      <Dialog
        icon="add"
        title="Add User"
        isOpen={isDialogOpen}
        onClose={() => setIsDialogOpen(false)}
      >
        <DialogBody>
          <FormGroup label="Username" labelInfo="(required)">
            <InputGroup
              placeholder="e.g. operator"
              value={newUsername}
              onChange={(e) => setNewUsername(e.target.value)}
            />
          </FormGroup>
          <FormGroup label="Password" labelInfo="(required)">
            <InputGroup
              type="password"
              placeholder="Password"
              value={newPassword}
              onChange={(e) => setNewPassword(e.target.value)}
            />
          </FormGroup>
          <FormGroup label="Roles" labelInfo="(required)">
            <RoleCheckboxes
              roles={roles}
              selectedRoleIds={selectedRoleIds}
              onChange={setSelectedRoleIds}
            />
          </FormGroup>
          {createUserMutation.isError && (
            <Callout intent="danger" icon="error">
              Failed to create user. Username may already exist.
            </Callout>
          )}
        </DialogBody>
        <DialogFooter
          actions={
            <>
              <Button onClick={() => setIsDialogOpen(false)}>Cancel</Button>
              <Button
                intent="primary"
                icon="add"
                onClick={handleCreate}
                loading={createUserMutation.isPending}
                disabled={!canCreateUser}
              >
                Add User
              </Button>
            </>
          }
        />
      </Dialog>

      <Dialog
        icon="shield"
        title={`Roles: ${editingUser?.username ?? ''}`}
        isOpen={!!editingUser}
        onClose={() => setEditingUser(null)}
      >
        <DialogBody>
          <RoleCheckboxes
            roles={roles}
            selectedRoleIds={editingRoleIds}
            onChange={setEditingRoleIds}
          />
          {setUserRolesMutation.isError && (
            <Callout intent="danger" icon="error">
              Failed to update roles. A tenant must keep at least one owner.
            </Callout>
          )}
        </DialogBody>
        <DialogFooter
          actions={
            <>
              <Button onClick={() => setEditingUser(null)}>Cancel</Button>
              <Button
                intent="primary"
                icon="tick"
                onClick={handleUpdateRoles}
                loading={setUserRolesMutation.isPending}
                disabled={editingRoleIds.length === 0 || setUserRolesMutation.isPending}
              >
                Save Roles
              </Button>
            </>
          }
        />
      </Dialog>
    </div>
  );
};

const RoleTags = ({ user }: { user: AuthUser }) => {
  const roles = user.roles && user.roles.length > 0 ? user.roles : undefined;

  if (!roles) {
    return <Tag minimal>{user.role}</Tag>;
  }

  return (
    <div className="role-tag-row">
      {roles.map((role) => (
        <Tag key={role.id} minimal intent={role.name === 'owner' ? 'primary' : 'none'}>
          {role.name}
        </Tag>
      ))}
    </div>
  );
};

const RoleCheckboxes = ({
  roles,
  selectedRoleIds,
  onChange,
}: {
  roles: Role[];
  selectedRoleIds: number[];
  onChange: (ids: number[]) => void;
}) => {
  const toggleRole = (roleId: number, checked: boolean) => {
    if (checked) {
      onChange([...selectedRoleIds, roleId]);
    } else {
      onChange(selectedRoleIds.filter((id) => id !== roleId));
    }
  };

  if (roles.length === 0) {
    return (
      <Callout intent="warning" icon="warning-sign">
        No roles are available.
      </Callout>
    );
  }

  return (
    <div className="role-checkbox-list">
      {roles.map((role) => (
        <Checkbox
          key={role.id}
          checked={selectedRoleIds.includes(role.id)}
          label={role.name}
          onChange={(event) => toggleRole(role.id, event.currentTarget.checked)}
        />
      ))}
    </div>
  );
};
