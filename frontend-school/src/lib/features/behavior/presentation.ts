import type {
	BehaviorSide,
	BehaviorSource,
	BehaviorScoreAction,
	BehaviorResetSchedule,
	BehaviorCleanupMode
} from '#lib/api/behavior.js';
export const sides: BehaviorSide[] = ['merit', 'deduction'];
export const sideLabels: Record<BehaviorSide, string> = {
	merit: 'คะแนนความดี',
	deduction: 'คะแนนที่ถูกหัก'
};
export const sources: BehaviorSource[] = ['manual', 'arrival', 'lesson', 'special'];
export const sourceLabels: Record<BehaviorSource, string> = {
	manual: 'ครูบันทึก',
	arrival: 'มาโรงเรียน',
	lesson: 'รายคาบ',
	special: 'รอบพิเศษ'
};
export const actions: BehaviorScoreAction[] = ['none', 'merit', 'deduction'];
export const actionLabels: Record<BehaviorScoreAction, string> = {
	none: 'ไม่คิดคะแนน',
	merit: 'เพิ่มความดี',
	deduction: 'หักคะแนน'
};
export const schedules: BehaviorResetSchedule[] = ['manual', 'monthly', 'term', 'year'];
export const scheduleLabels: Record<BehaviorResetSchedule, string> = {
	manual: 'รีเซ็ตเอง',
	monthly: 'ทุกเดือน',
	term: 'ทุกภาคเรียน',
	year: 'ทุกปีการศึกษา'
};
export const modes: BehaviorCleanupMode[] = ['details_only', 'archive', 'delete_effects'];
export const modeLabels: Record<BehaviorCleanupMode, string> = {
	details_only: 'ล้างรายละเอียด เก็บคะแนนไว้',
	archive: 'ย้ายคะแนนไปประวัติและล้างรายละเอียด',
	delete_effects: 'ลบรายละเอียดและผลคะแนน'
};
export function bangkokDateTime(date = new Date()): { date: string; time: string } {
	const parts = new Intl.DateTimeFormat('en-CA', {
		timeZone: 'Asia/Bangkok',
		year: 'numeric',
		month: '2-digit',
		day: '2-digit',
		hour: '2-digit',
		minute: '2-digit',
		hourCycle: 'h23'
	}).formatToParts(date);
	const part = (type: string) => parts.find((p) => p.type === type)?.value ?? '';
	return {
		date: `${part('year')}-${part('month')}-${part('day')}`,
		time: `${part('hour')}:${part('minute')}`
	};
}
export function occurredAt(date: string, time: string): string {
	return new Date(`${date}T${time}:00+07:00`).toISOString();
}
export function formatBehaviorTime(value: string): string {
	return new Intl.DateTimeFormat('th-TH', {
		timeZone: 'Asia/Bangkok',
		dateStyle: 'medium',
		timeStyle: 'short'
	}).format(new Date(value));
}
