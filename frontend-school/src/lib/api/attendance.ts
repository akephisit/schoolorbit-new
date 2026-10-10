import { apiClient, requireApiData, type ApiRequestOptions } from '#lib/api/client.js';
import type { components } from '#lib/api/generated/school-api.js';
type Schemas = components['schemas'];
export type AttendanceConfiguration = Schemas['AttendanceConfiguration'];
export type AttendanceSettings = Schemas['AttendanceSettings'];
export type AttendanceDay = Schemas['AttendanceDay'];
export type AttendanceSession = Schemas['AttendanceSession'];
export type AttendanceResult = Schemas['AttendanceResult'];
export type AttendanceKind = Schemas['AttendanceKind'];
export type AttendanceDetail = Schemas['AttendanceDetail'];
export type AttendanceWorkspace = Schemas['AttendanceWorkspace'];
export type AttendanceOptions = Schemas['AttendanceOptions'];
export type AttendanceReport = Schemas['AttendanceReport'];
export type AttendanceReportQuery = Schemas['AttendanceReportQuery'];
export type AttendanceHistoryItem = Schemas['AttendanceHistoryItem'];
export type AttendancePurgeImpact = Schemas['AttendancePurgeImpact'];
export type AttendanceKioskWorkspace = Schemas['AttendanceKioskWorkspace'];
export type AttendanceScanOutcome = Schemas['AttendanceScanOutcome'];
export type SaveAttendanceSettings = Schemas['SaveAttendanceSettings'];
export type SaveAttendanceDays = Schemas['SaveAttendanceDays'];
export type OpenAttendanceSession = Schemas['OpenAttendanceSession'];
export type SaveAttendanceResults = Schemas['SaveAttendanceResults'];
export type AttendanceCancellation = Schemas['AttendanceCancellation'];
export type SaveAttendanceAudience = Schemas['SaveAttendanceAudience'];
export type SpecialAttendanceDefinition = Schemas['SpecialAttendanceDefinition'];
export type SpecialAttendanceTemplate = Schemas['SpecialAttendanceTemplate'];
export type AttendanceAudienceGroup = Schemas['AttendanceAudienceGroup'];
export type SaveAttendanceDevice = Schemas['SaveAttendanceDevice'];
export type AttendanceDevice = Schemas['AttendanceDevice'];
export type EnrollAttendanceFace = Schemas['EnrollAttendanceFace'];
export type AttendanceScan = Schemas['AttendanceScan'];
export type PurgeAttendanceTerm = Schemas['PurgeAttendanceTerm'];
export type OpenAttendanceKiosk = Schemas['OpenAttendanceKiosk'];
export type AttendanceSummary = Schemas['AttendanceSummary'];
export type AttendanceStudentOption = Schemas['AttendanceStudentOption'];
export type AttendanceFace = Schemas['AttendanceFace'];
export type SpecialAttendanceGroup = Schemas['SpecialAttendanceGroup'];

const base = '/api/attendance';
const id = (value: string) => encodeURIComponent(value);
export function currentAttendanceDate(date = new Date()): string {
	return new Intl.DateTimeFormat('en-CA', {
		timeZone: 'Asia/Bangkok',
		year: 'numeric',
		month: '2-digit',
		day: '2-digit'
	}).format(date);
}
export const resultLabels: Record<AttendanceResult, string> = {
	unchecked: 'ยังไม่เช็ค',
	present: 'มา',
	late: 'สาย',
	absent: 'ขาด',
	leave: 'ลา',
	activity: 'กิจกรรม'
};
export const kindLabels: Record<AttendanceKind, string> = {
	arrival: 'เข้าโรงเรียน',
	flag: 'หน้าเสาธง',
	lesson: 'รายคาบ',
	special: 'รอบพิเศษ'
};
export async function attendanceWorkspace(
	academicTermId: string,
	date: string,
	options: ApiRequestOptions = {}
): Promise<AttendanceWorkspace> {
	return requireApiData(
		await apiClient.get<AttendanceWorkspace>(`${base}/workspace`, {
			...options,
			query: { academicTermId, date }
		}),
		'โหลดข้อมูลเช็คชื่อไม่สำเร็จ'
	);
}
export async function attendanceSettings(
	academicTermId: string,
	options: ApiRequestOptions = {}
): Promise<AttendanceSettings> {
	return requireApiData(
		await apiClient.get<AttendanceSettings>(`${base}/settings/${id(academicTermId)}`, {
			...options,
			query: undefined
		}),
		'โหลดข้อมูลเช็คชื่อไม่สำเร็จ'
	);
}
export async function attendanceDays(
	academicTermId: string,
	start: string,
	end: string,
	options: ApiRequestOptions = {}
): Promise<AttendanceDay[]> {
	return requireApiData(
		await apiClient.get<AttendanceDay[]>(`${base}/days`, {
			...options,
			query: { academicTermId, start, end }
		}),
		'โหลดข้อมูลเช็คชื่อไม่สำเร็จ'
	);
}
export async function attendanceOptions(
	academicTermId: string,
	options: ApiRequestOptions = {}
): Promise<AttendanceOptions> {
	return requireApiData(
		await apiClient.get<AttendanceOptions>(`${base}/options`, {
			...options,
			query: { academicTermId }
		}),
		'โหลดข้อมูลเช็คชื่อไม่สำเร็จ'
	);
}
export async function attendanceDetail(
	sessionId: string,
	options: ApiRequestOptions = {}
): Promise<AttendanceDetail> {
	return requireApiData(
		await apiClient.get<AttendanceDetail>(`${base}/sessions/${id(sessionId)}`, {
			...options,
			query: undefined
		}),
		'โหลดข้อมูลเช็คชื่อไม่สำเร็จ'
	);
}
export async function attendanceReport(
	academicTermId: string,
	studentId?: string,
	options: ApiRequestOptions = {},
	filters: Pick<AttendanceReportQuery, 'page' | 'pageSize' | 'search' | 'category'> = {}
): Promise<AttendanceReport> {
	return requireApiData(
		await apiClient.get<AttendanceReport>(`${base}/report`, {
			...options,
			query: { academicTermId, studentId, ...filters }
		}),
		'โหลดข้อมูลเช็คชื่อไม่สำเร็จ'
	);
}
export async function attendanceHistory(
	academicTermId: string,
	studentId: string,
	start: string,
	end: string,
	options: ApiRequestOptions = {}
): Promise<AttendanceHistoryItem[]> {
	return requireApiData(
		await apiClient.get<AttendanceHistoryItem[]>(`${base}/history`, {
			...options,
			query: { academicTermId, studentId, start, end }
		}),
		'โหลดข้อมูลเช็คชื่อไม่สำเร็จ'
	);
}
export async function attendancePurgeImpact(
	academicTermId: string,
	options: ApiRequestOptions = {}
): Promise<AttendancePurgeImpact> {
	return requireApiData(
		await apiClient.get<AttendancePurgeImpact>(`${base}/terms/${id(academicTermId)}/purge`, {
			...options,
			query: undefined
		}),
		'โหลดข้อมูลเช็คชื่อไม่สำเร็จ'
	);
}
export async function saveAttendanceSettings(
	academicTermId: string,
	payload: SaveAttendanceSettings
): Promise<AttendanceSettings> {
	return requireApiData(
		await apiClient.put<AttendanceSettings>(`${base}/settings/${id(academicTermId)}`, payload),
		'บันทึกเช็คชื่อไม่สำเร็จ'
	);
}
export async function saveAttendanceDays(
	academicTermId: string,
	payload: SaveAttendanceDays
): Promise<AttendanceSettings> {
	return requireApiData(
		await apiClient.put<AttendanceSettings>(`${base}/days/${id(academicTermId)}`, payload),
		'บันทึกเช็คชื่อไม่สำเร็จ'
	);
}
export async function openAttendanceSession(
	payload: OpenAttendanceSession
): Promise<AttendanceDetail> {
	return requireApiData(
		await apiClient.post<AttendanceDetail>(`${base}/sessions/open`, payload),
		'บันทึกเช็คชื่อไม่สำเร็จ'
	);
}
export async function saveAttendanceResults(
	sessionId: string,
	payload: SaveAttendanceResults
): Promise<AttendanceDetail> {
	return requireApiData(
		await apiClient.put<AttendanceDetail>(`${base}/sessions/${id(sessionId)}`, payload),
		'บันทึกเช็คชื่อไม่สำเร็จ'
	);
}
export async function cancelAttendance(
	sessionId: string,
	payload: AttendanceCancellation
): Promise<AttendanceDetail> {
	return requireApiData(
		await apiClient.put<AttendanceDetail>(
			`${base}/sessions/${id(sessionId)}/cancellation`,
			payload
		),
		'บันทึกเช็คชื่อไม่สำเร็จ'
	);
}
export async function saveAttendanceAudience(
	academicTermId: string,
	payload: SaveAttendanceAudience
): Promise<AttendanceAudienceGroup> {
	return requireApiData(
		await apiClient.post<AttendanceAudienceGroup>(
			`${base}/audiences/${id(academicTermId)}`,
			payload
		),
		'บันทึกเช็คชื่อไม่สำเร็จ'
	);
}
export async function createAttendanceSpecial(
	academicTermId: string,
	payload: SpecialAttendanceDefinition
): Promise<SpecialAttendanceTemplate> {
	return requireApiData(
		await apiClient.post<SpecialAttendanceTemplate>(
			`${base}/specials/${id(academicTermId)}`,
			payload
		),
		'บันทึกเช็คชื่อไม่สำเร็จ'
	);
}
export async function saveAttendanceDevice(
	deviceId: string,
	payload: SaveAttendanceDevice
): Promise<AttendanceDevice> {
	return requireApiData(
		await apiClient.put<AttendanceDevice>(`${base}/devices/${id(deviceId)}`, payload),
		'บันทึกเช็คชื่อไม่สำเร็จ'
	);
}
export async function enrollAttendanceFace(
	studentId: string,
	payload: EnrollAttendanceFace
): Promise<Record<string, never>> {
	return requireApiData(
		await apiClient.put<Record<string, never>>(`${base}/faces/${id(studentId)}`, payload),
		'บันทึกเช็คชื่อไม่สำเร็จ'
	);
}
export async function scanAttendance(payload: AttendanceScan): Promise<AttendanceScanOutcome> {
	return requireApiData(
		await apiClient.post<AttendanceScanOutcome>(`${base}/scans`, payload),
		'บันทึกเช็คชื่อไม่สำเร็จ'
	);
}
export async function openAttendanceKiosk(
	payload: OpenAttendanceKiosk
): Promise<AttendanceKioskWorkspace> {
	return requireApiData(
		await apiClient.post<AttendanceKioskWorkspace>(`${base}/kiosk/open`, payload),
		'บันทึกเช็คชื่อไม่สำเร็จ'
	);
}
export async function purgeAttendanceTerm(
	academicTermId: string,
	payload: PurgeAttendanceTerm
): Promise<AttendancePurgeImpact> {
	return requireApiData(
		await apiClient.post<AttendancePurgeImpact>(
			`${base}/terms/${id(academicTermId)}/purge`,
			payload
		),
		'บันทึกเช็คชื่อไม่สำเร็จ'
	);
}
export async function removeAttendanceFace(studentId: string) {
	return requireApiData(
		await apiClient.delete<Record<string, never>>(`${base}/faces/${id(studentId)}`),
		'ลบข้อมูลใบหน้าไม่สำเร็จ'
	);
}
