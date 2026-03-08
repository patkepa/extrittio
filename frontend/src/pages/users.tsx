import { Users as UsersIcon } from 'lucide-react';
import { EmptyState } from '@/components/ui/empty-state';

export function Users() {
  return (
    <div>
      <h3 className="text-lg font-semibold text-foreground">Users</h3>
      <p className="mt-1 text-sm text-muted">Manage team members and permissions.</p>
      <EmptyState
        icon={<UsersIcon size={48} />}
        title="User Management"
        description="Team member management will be available here."
        className="mt-8"
      />
    </div>
  );
}
