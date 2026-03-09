import { create } from 'zustand';

interface UIStore {
  isAddDeviceDialogOpen: boolean;
  openAddDeviceDialog: () => void;
  closeAddDeviceDialog: () => void;
  isCommandPaletteOpen: boolean;
  openCommandPalette: () => void;
  closeCommandPalette: () => void;
  toggleCommandPalette: () => void;
}

export const useUIStore = create<UIStore>((set) => ({
  isAddDeviceDialogOpen: false,
  openAddDeviceDialog: () => set({ isAddDeviceDialogOpen: true }),
  closeAddDeviceDialog: () => set({ isAddDeviceDialogOpen: false }),
  isCommandPaletteOpen: false,
  openCommandPalette: () => set({ isCommandPaletteOpen: true }),
  closeCommandPalette: () => set({ isCommandPaletteOpen: false }),
  toggleCommandPalette: () => set((s) => ({ isCommandPaletteOpen: !s.isCommandPaletteOpen })),
}));
