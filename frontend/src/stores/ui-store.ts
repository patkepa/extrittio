import { create } from 'zustand';

interface UIStore {
  isAddDeviceDialogOpen: boolean;
  openAddDeviceDialog: () => void;
  closeAddDeviceDialog: () => void;
  isCommandPaletteOpen: boolean;
  openCommandPalette: () => void;
  closeCommandPalette: () => void;
  toggleCommandPalette: () => void;
  isSidebarCollapsed: boolean;
  toggleSidebar: () => void;
  isRuleDialogOpen: boolean;
  editingRuleId: string | null;
  openRuleDialog: (ruleId?: string) => void;
  closeRuleDialog: () => void;
}

export const useUIStore = create<UIStore>((set) => ({
  isAddDeviceDialogOpen: false,
  openAddDeviceDialog: () => set({ isAddDeviceDialogOpen: true }),
  closeAddDeviceDialog: () => set({ isAddDeviceDialogOpen: false }),
  isCommandPaletteOpen: false,
  openCommandPalette: () => set({ isCommandPaletteOpen: true }),
  closeCommandPalette: () => set({ isCommandPaletteOpen: false }),
  toggleCommandPalette: () => set((s) => ({ isCommandPaletteOpen: !s.isCommandPaletteOpen })),
  isSidebarCollapsed: false,
  toggleSidebar: () => set((s) => ({ isSidebarCollapsed: !s.isSidebarCollapsed })),
  isRuleDialogOpen: false,
  editingRuleId: null,
  openRuleDialog: (ruleId?: string) =>
    set({ isRuleDialogOpen: true, editingRuleId: ruleId ?? null }),
  closeRuleDialog: () => set({ isRuleDialogOpen: false, editingRuleId: null }),
}));
