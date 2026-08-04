import client from './client';
import type {
  CaCertificateResponse,
  DeviceCertificateResponse,
  DeviceCertificateStatusResponse,
} from '../types/api';

export async function getCaCertificate(): Promise<CaCertificateResponse> {
  const { data } = await client.get<CaCertificateResponse>('/ca/certificate');
  return data;
}

export async function getDeviceCertificate(id: string): Promise<DeviceCertificateResponse> {
  const { data } = await client.get<DeviceCertificateResponse>(`/devices/${id}/certificate`);
  return data;
}

export async function getDeviceCertificateStatus(
  id: string,
): Promise<DeviceCertificateStatusResponse | null> {
  const { data } = await client.get<DeviceCertificateStatusResponse | null>(
    `/devices/${id}/certificate/status`,
  );
  return data;
}

export async function regenerateDeviceCertificate(id: string): Promise<DeviceCertificateResponse> {
  const { data } = await client.post<DeviceCertificateResponse>(
    `/devices/${id}/certificate/regenerate`,
  );
  return data;
}
