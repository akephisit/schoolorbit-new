import { z } from 'zod';
import { apiClient, BACKEND_URL, getSchoolSubdomainHint, requireApiData } from '#lib/api/client.js';
import type { components } from '#lib/api/generated/school-api.js';

type Schemas = components['schemas'];
export type TransferFile = Schemas['FileTransferFile'];
export type TransferDevice = Schemas['FileTransferDevice'];
export type TransferPair = Schemas['FileTransferPair'];
export type TransferSnapshot = Schemas['FileTransferSnapshot'];
export type TransferEvent = Schemas['FileTransferEvent'];
export type TransferTarget = Schemas['FileTransferTarget'];
export type TransferSignal = Schemas['FileTransferSignal'];
const prefix = '/api/staff/file-transfer';
const devicePath = (id: string) => `${prefix}/devices/${encodeURIComponent(id)}`;
const pairPath = (id: string) => `${prefix}/pairs/${encodeURIComponent(id)}`;
const options = (signal?: AbortSignal) => ({ signal });

const deviceSchema = z.object({
	id: z.string().uuid(),
	userId: z.string().uuid(),
	displayName: z.string(),
	label: z.string().refine((value) => [...value].length <= 60)
});
const fileSchema = z.object({
	name: z.string().refine((value) => [...value].length <= 255),
	size: z
		.number()
		.int()
		.min(0)
		.max(64 * 1024 * 1024),
	mimeType: z.string().refine((value) => [...value].length <= 128)
});
const pairSchema = z.object({
	id: z.string().uuid(),
	initiator: deviceSchema,
	recipient: deviceSchema,
	file: fileSchema,
	status: z.enum(['pending', 'accepted']),
	expiresAt: z.string()
});
const snapshotSchema = z.object({
	device: deviceSchema,
	pairingCode: z.string().regex(/^[A-F0-9]{10}$/),
	expiresAt: z.string(),
	peers: z.array(deviceSchema).max(1000),
	pairs: z.array(pairSchema).max(1)
});
const eventSchema = z.discriminatedUnion('type', [
	z.object({ type: z.literal('ready'), data: snapshotSchema }),
	z.object({ type: z.literal('presence_changed'), data: z.object({}) }),
	z.object({ type: z.literal('pair_changed'), data: pairSchema }),
	z.object({
		type: z.literal('pair_closed'),
		data: z.object({
			pairId: z.string().uuid(),
			reason: z.enum(['declined', 'cancelled', 'expired', 'disconnected'])
		})
	}),
	z.object({
		type: z.literal('signal'),
		data: z.object({
			pairId: z.string().uuid(),
			type: z.enum(['offer', 'answer']),
			sdp: z.string().max(32768)
		})
	}),
	z.object({ type: z.literal('session_invalid'), data: z.object({}) }),
	z.object({ type: z.literal('session_unavailable'), data: z.object({}) })
]);
export function parseTransferEvent(data: string): TransferEvent {
	if (data.length > 256 * 1024) throw new Error('ข้อมูลการเชื่อมต่อใหญ่เกินกำหนด');
	return eventSchema.parse(JSON.parse(data));
}
export const transferAPI = {
	async register(label: string, signal?: AbortSignal): Promise<TransferSnapshot> {
		return requireApiData(
			await apiClient.post<TransferSnapshot>(
				`${prefix}/devices`,
				{ label } satisfies Schemas['RegisterFileTransferDevice'],
				options(signal)
			),
			'เปิดรับส่งไฟล์ไม่สำเร็จ'
		);
	},
	async snapshot(deviceId: string, signal?: AbortSignal): Promise<TransferSnapshot> {
		return requireApiData(
			await apiClient.get<TransferSnapshot>(devicePath(deviceId), options(signal)),
			'โหลดอุปกรณ์ไม่สำเร็จ'
		);
	},
	async remove(deviceId: string): Promise<void> {
		requireApiData(
			await apiClient.delete<Schemas['EmptyData']>(devicePath(deviceId)),
			'ปิดอุปกรณ์ไม่สำเร็จ'
		);
	},
	async request(
		deviceId: string,
		target: TransferTarget,
		file: TransferFile,
		signal?: AbortSignal
	): Promise<TransferPair> {
		return requireApiData(
			await apiClient.post<TransferPair>(
				`${prefix}/pairs`,
				{ deviceId, target, file } satisfies Schemas['CreateFileTransferPair'],
				options(signal)
			),
			'ส่งคำขอไม่สำเร็จ'
		);
	},
	async accept(pairId: string, deviceId: string, signal?: AbortSignal): Promise<TransferPair> {
		return requireApiData(
			await apiClient.post<TransferPair>(
				`${pairPath(pairId)}/accept`,
				{ deviceId } satisfies Schemas['FileTransferDeviceRequest'],
				options(signal)
			),
			'ตอบรับไม่สำเร็จ'
		);
	},
	async close(pairId: string, deviceId: string): Promise<void> {
		requireApiData(
			await apiClient.post<Schemas['EmptyData']>(`${pairPath(pairId)}/close`, {
				deviceId
			} satisfies Schemas['FileTransferDeviceRequest']),
			'ยกเลิกไม่สำเร็จ'
		);
	},
	async signal(
		pairId: string,
		deviceId: string,
		type: Schemas['FileTransferSignalType'],
		sdp: string,
		signal?: AbortSignal
	): Promise<void> {
		requireApiData(
			await apiClient.post<Schemas['EmptyData']>(
				`${pairPath(pairId)}/signals`,
				{ deviceId, type, sdp } satisfies Schemas['SendFileTransferSignal'],
				options(signal)
			),
			'ส่งข้อมูลเชื่อมต่อไม่สำเร็จ'
		);
	},
	eventsUrl(deviceId: string): string {
		const url = new URL(`${BACKEND_URL}${devicePath(deviceId)}/events`);
		const school = getSchoolSubdomainHint();
		if (school) url.searchParams.set('school_subdomain', school);
		return url.toString();
	}
};
