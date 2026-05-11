import { useRef, useCallback } from 'react';
import { QRCodeSVG, QRCodeCanvas } from 'qrcode.react';
import { Button } from '@blueprintjs/core';
import { showSuccessToast, showErrorToast } from '../../utils/toaster';
import './qr-code-card.css';

interface QrCodeCardProps {
  deviceId: string;
}

export const QrCodeCard = ({ deviceId }: QrCodeCardProps) => {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const url = `${window.location.origin}/devices/${deviceId}`;

  const handleCopyUrl = useCallback(async () => {
    try {
      await navigator.clipboard.writeText(url);
      void showSuccessToast('URL copied to clipboard');
    } catch {
      void showErrorToast('Failed to copy URL');
    }
  }, [url]);

  const handleCopyQr = useCallback(async () => {
    const canvas = canvasRef.current;
    if (!canvas) {
      void showErrorToast('QR canvas not available');
      return;
    }

    try {
      const blob = await new Promise<Blob | null>((resolve) => canvas.toBlob(resolve, 'image/png'));
      if (!blob) {
        void showErrorToast('Failed to generate QR image');
        return;
      }
      await navigator.clipboard.write([new ClipboardItem({ 'image/png': blob })]);
      void showSuccessToast('QR code copied to clipboard');
    } catch {
      void showErrorToast('Failed to copy QR image — try right-clicking to save instead');
    }
  }, []);

  const handleDownloadQr = useCallback(() => {
    const canvas = canvasRef.current;
    if (!canvas) {
      void showErrorToast('QR canvas not available');
      return;
    }

    const dataUrl = canvas.toDataURL('image/png');
    const link = document.createElement('a');
    link.download = `device-${deviceId}-qr.png`;
    link.href = dataUrl;
    document.body.appendChild(link);
    link.click();
    document.body.removeChild(link);
  }, [deviceId]);

  return (
    <div className="qr-code-card">
      <span className="section-label">QR Code</span>

      <div className="qr-code-wrapper">
        <QRCodeSVG value={url} size={160} />
      </div>

      <div className="qr-code-url">{url}</div>

      <div className="qr-code-actions">
        <Button icon="link" size="small" onClick={() => void handleCopyUrl()}>
          Copy URL
        </Button>
        <Button icon="media" size="small" onClick={() => void handleCopyQr()}>
          Copy QR
        </Button>
        <Button icon="download" size="small" onClick={handleDownloadQr}>
          Download
        </Button>
      </div>

      <div className="qr-canvas-hidden">
        <QRCodeCanvas ref={canvasRef} value={url} size={320} />
      </div>
    </div>
  );
};
