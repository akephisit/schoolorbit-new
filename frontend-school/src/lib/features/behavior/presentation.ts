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
export const schedules: BehaviorResetSchedule[] = ['manual', 'monthly', 'scheduled'];
export const scheduleLabels: Record<BehaviorResetSchedule, string> = {
	manual: 'รีเซ็ตเอง',
	monthly: 'ทุกเดือน',
	scheduled: 'กำหนดวันที่สิ้นสุดรอบ'
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
		second: '2-digit',
		hourCycle: 'h23'
	}).formatToParts(date);
	const part = (type: string) => parts.find((p) => p.type === type)?.value ?? '';
	return {
		date: `${part('year')}-${part('month')}-${part('day')}`,
		time: `${part('hour')}:${part('minute')}:${part('second')}`
	};
}
export function occurredAt(date: string, time: string): string {
	return new Date(`${date}T${time.length === 5 ? time + ':00' : time}+07:00`).toISOString();
}
export function formatBehaviorTime(value: string): string {
	return new Intl.DateTimeFormat('th-TH', {
		timeZone: 'Asia/Bangkok',
		dateStyle: 'medium',
		timeStyle: 'short'
	}).format(new Date(value));
}

export function resolveBehaviorOccurredAt(
	date: string,
	time: string,
	defaultInstant: string
): string {
	const clock = bangkokDateTime(new Date(defaultInstant));
	return date === clock.date && time === clock.time ? defaultInstant : occurredAt(date, time);
}

export function formatBehaviorDate(value: string): string {
	return new Intl.DateTimeFormat('th-TH', {
		timeZone: 'Asia/Bangkok',
		dateStyle: 'medium'
	}).format(new Date(`${value}T12:00:00+07:00`));
}
