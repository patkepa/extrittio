import { create } from "zustand";
import type { BulkDeviceFilters } from "../types/api";

interface SelectionStore {
  selectedDeviceIds: Set<string>;
  isAllMatchingSelected: boolean;
  selectionFilters: BulkDeviceFilters | null;

  toggleDevice: (id: string) => void;
  selectAllVisible: (ids: string[]) => void;
  deselectAllVisible: () => void;
  selectAllMatching: (filters: BulkDeviceFilters) => void;
  clearSelection: () => void;
  isSelected: (id: string) => boolean;
}

export const useSelectionStore = create<SelectionStore>((set, get) => ({
  selectedDeviceIds: new Set(),
  isAllMatchingSelected: false,
  selectionFilters: null,

  toggleDevice: (id) =>
    set((state) => {
      const next = new Set(state.selectedDeviceIds);
      if (next.has(id)) {
        next.delete(id);
      } else {
        next.add(id);
      }
      return {
        selectedDeviceIds: next,
        isAllMatchingSelected: false,
        selectionFilters: null,
      };
    }),

  selectAllVisible: (ids) =>
    set({
      selectedDeviceIds: new Set(ids),
      isAllMatchingSelected: false,
      selectionFilters: null,
    }),

  deselectAllVisible: () =>
    set({
      selectedDeviceIds: new Set(),
      isAllMatchingSelected: false,
      selectionFilters: null,
    }),

  selectAllMatching: (filters) =>
    set({
      selectedDeviceIds: new Set(),
      isAllMatchingSelected: true,
      selectionFilters: filters,
    }),

  clearSelection: () =>
    set({
      selectedDeviceIds: new Set(),
      isAllMatchingSelected: false,
      selectionFilters: null,
    }),

  isSelected: (id) => get().selectedDeviceIds.has(id),
}));
