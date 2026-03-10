import { OverlayToaster, Intent } from '@blueprintjs/core';

const toaster = OverlayToaster.createAsync({
  position: 'top',
  maxToasts: 3,
});

export async function showSuccessToast(message: string) {
  (await toaster).show({
    message,
    intent: Intent.SUCCESS,
    icon: 'tick',
    timeout: 3000,
  });
}

export async function showErrorToast(message: string) {
  (await toaster).show({
    message,
    intent: Intent.DANGER,
    icon: 'error',
    timeout: 5000,
  });
}

export async function showWarningToast(message: string) {
  (await toaster).show({
    message,
    intent: Intent.WARNING,
    icon: 'warning-sign',
    timeout: 4000,
  });
}
