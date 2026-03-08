import { create } from 'zustand';

interface UIStore {
  isAddDeviceDialogOpen: boolean;
  openAddDeviceDialog: () => void;
  closeAddDeviceDialog: () => void;
}

export const useUIStore = create<UIStore>((set) => ({
  isAddDeviceDialogOpen: false,
  openAddDeviceDialog: () => set({ isAddDeviceDialogOpen: true }),
  closeAddDeviceDialog: () => set({ isAddDeviceDialogOpen: false }),
}));
