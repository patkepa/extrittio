import { Card } from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { Switch } from '@/components/ui/switch';

export function Settings() {
  return (
    <div className="max-w-3xl space-y-6">
      <div>
        <h3 className="text-lg font-semibold text-foreground">Settings</h3>
        <p className="mt-1 text-sm text-muted">Application configuration</p>
      </div>

      <Card className="p-5 border-l-2 border-l-accent">
        <span className="text-[11px] font-bold uppercase tracking-widest text-muted mb-4 pb-2 border-b border-border block">
          Profile
        </span>
        <div className="space-y-4">
          <div>
            <label htmlFor="name-input" className="block text-sm font-semibold text-foreground mb-1.5">
              Name
            </label>
            <Input id="name-input" placeholder="Enter your name" defaultValue="satnaing" />
          </div>
          <div>
            <label htmlFor="email-input" className="block text-sm font-semibold text-foreground mb-1.5">
              Email
            </label>
            <Input id="email-input" type="email" placeholder="Enter your email" defaultValue="satnaingdev@gmail.com" />
          </div>
        </div>
      </Card>

      <Card className="p-5 border-l-2 border-l-accent">
        <span className="text-[11px] font-bold uppercase tracking-widest text-muted mb-4 pb-2 border-b border-border block">
          Notifications
        </span>
        <div className="space-y-3">
          <Switch label="Email notifications" defaultChecked />
          <Switch label="Push notifications" />
          <Switch label="Device alerts" defaultChecked />
        </div>
      </Card>

      <Card className="p-5 border-l-2 border-l-accent">
        <span className="text-[11px] font-bold uppercase tracking-widest text-muted mb-4 pb-2 border-b border-border block">
          Display
        </span>
        <div className="space-y-3">
          <Switch label="Compact mode" />
          <Switch label="Show device thumbnails" defaultChecked />
          <Switch label="Enable animations" defaultChecked />
        </div>
      </Card>
    </div>
  );
}
