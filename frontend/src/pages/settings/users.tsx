import { useState } from 'react';
import { showSuccessToast, showErrorToast } from '../../utils/toaster';
import {
  Card,
  Elevation,
  H3,
  HTMLTable,
  Button,
  Dialog,
  DialogBody,
  DialogFooter,
  FormGroup,
  InputGroup,
  Callout,
} from '@blueprintjs/core';
import { useUsers, useCreateUser, useDeleteUser } from '../../hooks/use-users';

export const UsersSettings = () => {
  const { data: users = [], isLoading } = useUsers();
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
        onError: () => { void showErrorToast('Failed to create user'); },
      }
    );
  };

  return (
    <div>
      <div className="page-header">
        <div>
          <H3>Users</H3>
          <p className="page-description">Manage user accounts</p>
        </div>
        <Button intent="primary" icon="add" onClick={() => setIsDialogOpen(true)}>
          Add User
        </Button>
      </div>

      <Card elevation={Elevation.ONE}>
        {isLoading ? (
          <p>Loading...</p>
        ) : (
          <HTMLTable interactive style={{ width: '100%' }}>
            <thead>
              <tr>
                <th>Username</th>
                <th>Role</th>
                <th style={{ width: 80 }}>Actions</th>
              </tr>
            </thead>
            <tbody>
              {users.map((user) => (
                <tr key={user.id}>
                  <td><strong>{user.username}</strong></td>
                  <td>{user.role}</td>
                  <td>
                    <Button
                      icon="trash"
                      minimal
                      small
                      intent="danger"
                      disabled={users.length <= 1}
                      loading={deleteUserMutation.isPending && deleteUserMutation.variables === user.id}
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
                disabled={!newUsername.trim() || !newPassword.trim()}
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
