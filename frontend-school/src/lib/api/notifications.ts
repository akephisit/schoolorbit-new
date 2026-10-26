import { apiClient, requireApiData } from '#lib/api/client.js';
import type { components } from '#lib/api/generated/school-api.js';

type Schemas = components['schemas'];
export type PushConfig = Schemas['PushConfigResponse'];
export type PushDevice = Schemas['PushDevice'];
type ListPushDevicesResponse = Schemas['ListPushDevicesResponse'];
type RegisterPushDeviceRequest = Schemas['RegisterPushDeviceRequest'];

export async function getPushConfig(): Promise<PushConfig> {
	return requireApiData(
		await apiClient.get<PushConfig>('/api/notifications/push/config'),
		'ไม่สามารถตรวจสอบระบบแจ้งเตือนได้'
	);
}
export async function listPushDevices(): Promise<PushDevice[]> {
	return requireApiData(
		await apiClient.get<ListPushDevicesResponse>('/api/notifications/push/devices'),
		'ไม่สามารถโหลดอุปกรณ์ได้'
	).items;
}
export async function registerPushDevice(body: RegisterPushDeviceRequest): Promise<PushDevice> {
	return requireApiData(
		await apiClient.post<PushDevice>('/api/notifications/push/devices', body),
		'ไม่สามารถเชื่อมต่ออุปกรณ์ได้'
	);
}
export async function unregisterPushDevice(id: string, version: string): Promise<void> {
	requireApiData(
		await apiClient.delete<Schemas['EmptyData']>(
			`/api/notifications/push/devices/${encodeURIComponent(id)}?version=${encodeURIComponent(version)}`
		),
		'ไม่สามารถยกเลิกอุปกรณ์ได้'
	);
}
