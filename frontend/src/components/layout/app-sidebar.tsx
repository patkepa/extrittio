import { useState } from 'react';
import { useNavigate, useLocation } from 'react-router-dom';
import {
  Box,
  ChevronDown,
  ChevronRight,
  ChevronsUpDown,
  Check,
  LogOut,
} from 'lucide-react';
import { cn } from '@/lib/utils';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import { Tooltip } from '@/components/ui/tooltip';
import { Collapsible, CollapsibleContent } from '@/components/ui/collapsible';
import { navGroups, projects, currentUser } from '../../data/sidebar-data';
import type { NavItem } from '../../types/navigation';

interface AppSidebarProps {
  isCollapsed?: boolean;
  isMobileOpen?: boolean;
  onMobileClose?: () => void;
}

const navBadges: Record<string, { count?: number; status?: 'online' | 'warning' | 'offline' }> = {
  Dashboard: { status: 'online' },
  Devices: { count: 987 },
  Alerts: { count: 3 },
};

const envBorderColors: Record<string, string> = {
  Development: 'border-accent',
  Testing: 'border-warning',
  Production: 'border-success',
};

const envDotColors: Record<string, string> = {
  Development: 'bg-accent',
  Testing: 'bg-warning',
  Production: 'bg-success',
};

export const AppSidebar = ({
  isCollapsed = false,
  isMobileOpen = false,
  onMobileClose,
}: AppSidebarProps) => {
  const navigate = useNavigate();
  const location = useLocation();
  const [expandedItems, setExpandedItems] = useState<Set<string>>(new Set());
  const [selectedProject, setSelectedProject] = useState(projects[0]!);
  const [projectSwitcherOpen, setProjectSwitcherOpen] = useState(false);

  const toggleExpanded = (label: string) => {
    setExpandedItems((prev) => {
      const next = new Set(prev);
      if (next.has(label)) {
        next.delete(label);
      } else {
        next.add(label);
      }
      return next;
    });
  };

  const isActive = (href: string) => {
    if (href === '/') return location.pathname === '/';
    return location.pathname.startsWith(href);
  };

  const handleNavigation = (href: string) => {
    navigate(href);
    onMobileClose?.();
  };

  const renderNavItem = (item: NavItem, depth: number = 0) => {
    const hasChildren = item.children && item.children.length > 0;
    const isExpanded = expandedItems.has(item.label);
    const active = isActive(item.href);
    const badge = navBadges[item.label];
    const IconComponent = item.icon;

    const navButton = (
      <button
        onClick={() => {
          if (hasChildren) {
            toggleExpanded(item.label);
          } else {
            handleNavigation(item.href);
          }
        }}
        aria-expanded={hasChildren ? isExpanded : undefined}
        className={cn(
          'flex items-center w-full gap-2.5 px-3 py-2 rounded-md text-[13px] font-medium tracking-tight',
          'border border-transparent transition-all duration-100 cursor-pointer',
          'text-muted hover:text-foreground hover:bg-surface-hover',
          active && 'bg-accent-muted text-accent border border-accent/25 font-semibold',
          isCollapsed && 'justify-center px-2',
        )}
      >
        <IconComponent size={16} className="shrink-0" />
        {!isCollapsed && (
          <>
            <span className="flex-1 text-left truncate">{item.label}</span>
            <span className="flex items-center gap-1.5">
              {badge &&
                (badge.status ? (
                  <span className={`status-led status-led--${badge.status}`} />
                ) : badge.count ? (
                  <Badge variant="accent" round className="min-w-[24px] h-[18px] font-mono font-bold">
                    {badge.count}
                  </Badge>
                ) : null)}
              {hasChildren &&
                (isExpanded ? (
                  <ChevronDown size={12} className="text-muted" />
                ) : (
                  <ChevronRight size={12} className="text-muted" />
                ))}
            </span>
          </>
        )}
      </button>
    );

    return (
      <div key={item.label} style={{ paddingLeft: depth > 0 ? `${depth * 16}px` : undefined }}>
        {isCollapsed ? (
          <Tooltip content={item.label} side="right">
            {navButton}
          </Tooltip>
        ) : (
          navButton
        )}
        {hasChildren && !isCollapsed && (
          <Collapsible open={isExpanded}>
            <CollapsibleContent>
              <div className="ml-5 border-l-2 border-accent/15 pl-1 mt-0.5 mb-1">
                {item.children!.map((child) => renderNavItem(child, depth + 1))}
              </div>
            </CollapsibleContent>
          </Collapsible>
        )}
      </div>
    );
  };

  return (
    <div
      className={cn(
        'flex flex-col h-screen bg-surface border-r border-border shrink-0 overflow-hidden',
        'transition-[width] duration-250 ease-[cubic-bezier(0.4,0,0.2,1)]',
        isCollapsed ? 'w-[60px]' : 'w-[260px]',
        isMobileOpen && 'mobile-open',
      )}
    >
      {/* Header */}
      <div
        className={cn(
          'p-4 border-b border-border',
          isCollapsed && 'flex justify-center p-3.5',
        )}
      >
        <div
          className={cn(
            'flex items-center gap-2.5 text-foreground text-lg font-bold tracking-tight',
            isCollapsed && 'justify-center',
          )}
        >
          <div
            className={cn(
              'flex items-center justify-center rounded-lg bg-gradient-to-br from-accent to-accent-hover text-white shrink-0 shadow-[0_2px_8px_rgba(45,114,210,0.3)]',
              isCollapsed ? 'w-7 h-7' : 'w-8 h-8',
            )}
          >
            <Box size={isCollapsed ? 14 : 18} />
          </div>
          {!isCollapsed && <span className="font-bold text-foreground whitespace-nowrap">Extrittio</span>}
        </div>
      </div>

      {/* Project Switcher */}
      {!isCollapsed && (
        <div className="p-3 border-b border-border">
          <Collapsible open={projectSwitcherOpen} onOpenChange={setProjectSwitcherOpen}>
            <button
              onClick={() => setProjectSwitcherOpen(!projectSwitcherOpen)}
              className="flex items-center justify-between w-full px-3 py-2.5 bg-surface-hover border border-border rounded-lg cursor-pointer transition-all duration-150 hover:border-accent/30 text-foreground"
            >
              <div className="flex items-center gap-2.5">
                <div
                  className={cn(
                    'w-[30px] h-[30px] rounded-md bg-[hsl(0_0%_12%)] border-[1.5px] flex items-center justify-center shrink-0 text-foreground',
                    envBorderColors[selectedProject.environment],
                  )}
                >
                  <selectedProject.icon size={14} />
                </div>
                <div className="flex flex-col items-start gap-px">
                  <span className="text-[13px] font-bold text-foreground leading-tight">
                    {selectedProject.name}
                  </span>
                  <span className="text-[9px] text-muted tracking-widest font-mono">
                    {selectedProject.environment.toUpperCase()}
                  </span>
                </div>
              </div>
              <ChevronsUpDown size={14} className="text-muted" />
            </button>
            <CollapsibleContent>
              <div className="flex flex-col gap-0.5 pt-2">
                {projects.map((project) => {
                  const ProjectIcon = project.icon;
                  return (
                    <button
                      key={project.environment}
                      className={cn(
                        'flex items-center gap-2.5 w-full px-3 py-2 bg-transparent border border-transparent rounded-md cursor-pointer transition-all duration-100 text-foreground',
                        'hover:bg-surface-hover hover:border-border',
                        selectedProject.environment === project.environment &&
                          'bg-accent-muted border-accent/20',
                      )}
                      onClick={() => {
                        setSelectedProject(project);
                        setProjectSwitcherOpen(false);
                      }}
                    >
                      <div
                        className={cn(
                          'w-6 h-6 rounded-[5px] bg-[hsl(0_0%_12%)] border-[1.5px] flex items-center justify-center shrink-0 text-foreground',
                          envBorderColors[project.environment],
                        )}
                      >
                        <ProjectIcon size={12} />
                      </div>
                      <div className="flex flex-col items-start flex-1">
                        <span className="text-xs font-semibold text-foreground leading-tight">
                          {project.name}
                        </span>
                        <span className="text-[9px] text-muted tracking-wider font-mono">
                          {project.environment}
                        </span>
                      </div>
                      {selectedProject.environment === project.environment && (
                        <Check size={14} className="text-accent" />
                      )}
                    </button>
                  );
                })}
              </div>
            </CollapsibleContent>
          </Collapsible>
        </div>
      )}

      {/* Navigation */}
      <div className="flex-1 overflow-y-auto overflow-x-hidden py-2 scrollbar-thin">
        {navGroups.map((group, idx) => (
          <div key={group.label} className="mb-2">
            {!isCollapsed && (
              <div className="px-5 pt-2.5 pb-1 text-[10px] font-bold text-muted tracking-widest uppercase opacity-70">
                {group.label}
              </div>
            )}
            {isCollapsed && idx > 0 && (
              <div className="mx-3 my-2 border-t border-border opacity-50" />
            )}
            <div className="px-2 space-y-px">
              {group.items.map((item) => renderNavItem(item))}
            </div>
          </div>
        ))}
      </div>

      {/* Footer */}
      <div
        className={cn(
          'p-3 border-t border-border bg-surface',
          isCollapsed && 'flex flex-col items-center px-2',
        )}
      >
        {!isCollapsed ? (
          <>
            {/* Environment badge */}
            <div className="flex items-center gap-2 px-2.5 py-1.5 mb-2 rounded-md bg-surface-hover border border-border">
              <span
                className={cn('w-1.5 h-1.5 rounded-full shrink-0', envDotColors[selectedProject.environment])}
              />
              <span className="text-[9px] tracking-widest text-foreground font-mono">
                {selectedProject.environment.toUpperCase()}
              </span>
              <span className="text-[9px] text-muted font-mono ml-auto">v0.1.0</span>
            </div>

            {/* User card */}
            <div className="group p-2.5 bg-surface-hover border border-border rounded-lg transition-colors hover:border-accent/20">
              <div className="flex items-center gap-2.5">
                <div className="w-8 h-8 rounded-lg bg-gradient-to-br from-accent to-accent-hover flex items-center justify-center text-white shrink-0 font-bold text-[13px] shadow-[0_2px_6px_rgba(45,114,210,0.25)]">
                  {currentUser.name.charAt(0).toUpperCase()}
                </div>
                <div className="flex-1 min-w-0">
                  <div className="text-[13px] font-semibold text-foreground truncate tracking-tight leading-tight">
                    {currentUser.name}
                  </div>
                  <div className="text-[11px] font-medium text-muted truncate">
                    {currentUser.email}
                  </div>
                </div>
                <Button
                  variant="ghost"
                  size="sm"
                  title="Sign out"
                  className="h-7 w-7 p-0 opacity-0 group-hover:opacity-60 hover:!opacity-100 transition-opacity"
                >
                  <LogOut size={14} />
                </Button>
              </div>
            </div>
          </>
        ) : (
          <Tooltip content={currentUser.name} side="right">
            <div className="w-8 h-8 rounded-lg bg-gradient-to-br from-accent to-accent-hover flex items-center justify-center text-white shrink-0 font-bold text-[13px] shadow-[0_2px_6px_rgba(45,114,210,0.25)] cursor-pointer mx-auto">
              {currentUser.name.charAt(0).toUpperCase()}
            </div>
          </Tooltip>
        )}
      </div>
    </div>
  );
};
