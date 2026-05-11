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
} from '@blueprintjs/core';
import { useConfirmShortcut } from '@extrittio/interactions';
import { useUsers, useCreateUser, useDeleteUser } from '../../hooks/use-users';
import './settings.css';

export const UsersSettings = () => {
  const { data: users = [], isLoading, error } = useUsers();
  const createUserMutation = useCreateUser();
  const deleteUserMutation = useDeleteUser();
  const [isDialogOpen, setIsDialogOpen] = useState(false);
  const [newUsername, setNewUsername] = useState('');
  const [newPassword, setNewPassword] = useState('');

  const handleCreate = () => {
    createUserMutation.mutate(
      { username: newUsername, password: newPassword },
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
    !!newUsername.trim() && !!newPassword.trim() && !createUserMutation.isPending;

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
        <Button intent="primary" icon="add" onClick={() => setIsDialogOpen(true)}>
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
                  <td>{user.role}</td>
                  <td className="actions-column">
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
    </div>
  );
};
