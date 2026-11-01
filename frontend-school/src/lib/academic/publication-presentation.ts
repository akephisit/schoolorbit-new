import type { AcademicTermChangeSet, DeliveryVersionSummary } from '#lib/api/learning-delivery.js';
import type { TimetableBlockWorkspace, TimetableVersion } from '#lib/api/timetable.js';

export function publicationDate(value: string): string {
	return new Intl.DateTimeFormat('th-TH', { dateStyle: 'medium', timeZone: 'Asia/Bangkok' }).format(
		new Date(`${value}T00:00:00+07:00`)
	);
}

export function publicationTime(value: string): string {
	return new Intl.DateTimeFormat('th-TH', {
		dateStyle: 'short',
		timeStyle: 'short',
		timeZone: 'Asia/Bangkok'
	}).format(new Date(value));
}

export function publicationEffectiveLabel(
	version: Pick<DeliveryVersionSummary, 'effectiveFrom' | 'effectiveUntil' | 'isSuperseded'>
): string {
	if (version.isSuperseded) return 'แผนนี้ถูกแทนที่แล้ว';
	if (!version.effectiveFrom) return 'เลือกวันเริ่มใช้ตอนเผยแพร่';
	return `เริ่มใช้ ${publicationDate(version.effectiveFrom)}${version.effectiveUntil ? ` ถึง ${publicationDate(version.effectiveUntil)}` : ''}`;
}

export function deliveryPublicationSummaryLabel(
	version: Pick<DeliveryVersionSummary, 'status' | 'publicationNo' | 'isSuperseded'>
): string {
	const status = version.isSuperseded
		? 'ถูกแทนที่'
		: version.status === 'draft'
			? 'แบบร่าง'
			: version.status === 'published'
				? 'เผยแพร่แล้ว'
				: 'ยกเลิก';
	return [status, version.publicationNo === null ? null : `รุ่นที่ ${version.publicationNo}`]
		.filter(Boolean)
		.join(' · ');
}

export function deliveryPublicationLabel(
	version: Pick<
		DeliveryVersionSummary,
		'status' | 'publicationNo' | 'effectiveFrom' | 'publishedAt' | 'isSuperseded'
	>
): string {
	return [
		deliveryPublicationSummaryLabel(version),
		version.effectiveFrom
			? `เริ่มใช้ ${publicationDate(version.effectiveFrom)}`
			: 'เลือกวันเริ่มใช้ตอนเผยแพร่',
		version.publishedAt ? `เผยแพร่ ${publicationTime(version.publishedAt)}` : null
	]
		.filter(Boolean)
		.join(' · ');
}

export function timetablePublicationLabel(version: TimetableVersion): string {
	return deliveryPublicationLabel({
		...version,
		isSuperseded: version.displayState === 'superseded'
	});
}

export function publicationSourceLabel(
	source: TimetableBlockWorkspace['deliveryPublication']
): string {
	return `รุ่นที่ ${source.publicationNo} · เริ่มใช้ ${publicationDate(source.effectiveFrom)} · ${source.publishedAt ? `เผยแพร่ ${publicationTime(source.publishedAt)}` : 'ไม่พบเวลาเผยแพร่ที่บันทึกไว้'}`;
}

export function publicationDateRulesLabel(
	rules: AcademicTermChangeSet['publicationDateRules']
): string {
	return rules.minimumEffectiveFrom
		? `เลือกวันเริ่มใช้ได้ตั้งแต่ ${publicationDate(rules.minimumEffectiveFrom)}`
		: 'เลือกวันเริ่มใช้ตามแผนที่ต้องการ ระบบจะตรวจประวัติที่ได้รับผลกระทบก่อนเผยแพร่';
}
