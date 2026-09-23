import { useEffect, useRef, useState } from 'react';
import { Icon } from '@blueprintjs/core';
import type { User } from '../../types/navigation';
import './user-settings-menu.css';

interface UserSettingsMenuProps {
  user: User;
  collapsed: boolean;
  onNavigate: (path: string) => void;
  onLogout: () => void;
}

export const UserSettingsMenu = ({
  user,
  collapsed,
  onNavigate,
  onLogout,
}: UserSettingsMenuProps) => {
  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const closeOnOutsideClick = (event: PointerEvent) => {
      if (!rootRef.current?.contains(event.target as Node)) setOpen(false);
    };
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key === 'Escape') setOpen(false);
    };
    document.addEventListener('pointerdown', closeOnOutsideClick);
    document.addEventListener('keydown', closeOnEscape);
    return () => {
      document.removeEventListener('pointerdown', closeOnOutsideClick);
      document.removeEventListener('keydown', closeOnEscape);
    };
  }, [open]);

  const goTo = (path: string) => {
    setOpen(false);
    onNavigate(path);
  };

  return (
    <div
      className={`user-settings-menu ${collapsed ? 'user-settings-menu--collapsed' : ''}`}
      ref={rootRef}
    >
      <button
        className="user-settings-trigger"
        type="button"
        aria-label={`${open ? 'Close' : 'Open'} settings for ${user.name}`}
        aria-controls="user-settings-panel"
        aria-expanded={open}
        onClick={() => setOpen((value) => !value)}
      >
        <span className="user-avatar">{user.name.charAt(0).toUpperCase()}</span>
        {!collapsed && (
          <>
            <span className="user-details">
              <span className="user-name">{user.name}</span>
              {user.email && <span className="user-email">{user.email}</span>}
            </span>
            <Icon icon={open ? 'chevron-up' : 'chevron-down'} size={12} />
          </>
        )}
      </button>
      <div
        className={`user-settings-panel ${open ? 'user-settings-panel--open' : ''}`}
        id="user-settings-panel"
        aria-hidden={!open}
        inert={!open}
      >
        <div className="user-settings-panel-heading">User settings</div>
        <button
          className="user-settings-action"
          type="button"
          onClick={() => goTo('/settings/profile')}
        >
          <Icon icon="user" size={14} />
          <span>Profile</span>
        </button>
        <button className="user-settings-action" type="button" onClick={() => goTo('/help')}>
          <Icon icon="help" size={14} />
          <span>Help</span>
        </button>
        <div className="user-settings-divider" />
        <button className="user-settings-action" type="button" onClick={onLogout}>
          <Icon icon="log-out" size={14} />
          <span>Sign out</span>
        </button>
      </div>
    </div>
  );
};
