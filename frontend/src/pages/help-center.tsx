import { HelpCircle } from 'lucide-react';
import { EmptyState } from '@/components/ui/empty-state';

export function HelpCenter() {
  return (
    <div>
      <h3 className="text-lg font-semibold text-foreground">Help Center</h3>
      <p className="mt-1 text-sm text-muted">Documentation and support resources.</p>
      <EmptyState
        icon={<HelpCircle size={48} />}
        title="Help Center"
        description="Documentation and support resources will be available here."
        className="mt-8"
      />
    </div>
  );
}
