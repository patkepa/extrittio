import { H3, NonIdealState } from '@blueprintjs/core';

export const Users = () => {
  return (
    <div className="users-page">
      <div className="page-header">
        <H3>Users</H3>
      </div>
      <NonIdealState
        icon="people"
        title="Users"
        description="User management coming soon."
      />
    </div>
  );
};
