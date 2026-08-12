import { useEffect } from "react";

export interface UseConfirmShortcutOptions {
  isOpen: boolean;
  canConfirm?: boolean;
  onConfirm: () => void;
}

export function useConfirmShortcut({
  isOpen,
  canConfirm = true,
  onConfirm,
}: UseConfirmShortcutOptions) {
  useEffect(() => {
    if (!isOpen || !canConfirm) {
      return;
    }

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.defaultPrevented || event.isComposing) {
        return;
      }

      if ((event.metaKey || event.ctrlKey) && event.key === "Enter") {
        event.preventDefault();
        onConfirm();
      }
    };

    document.addEventListener("keydown", handleKeyDown);
    return () => document.removeEventListener("keydown", handleKeyDown);
  }, [canConfirm, isOpen, onConfirm]);
}
