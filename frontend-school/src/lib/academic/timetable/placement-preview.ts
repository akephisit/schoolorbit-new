import type { TimetableBlockPlacementPreview } from '../../api/timetable';

export function placementFailureMessage(preview: TimetableBlockPlacementPreview): string | null {
	if (preview.state === 'blocked') {
		const reasons = preview.conflicts.map((conflict) => conflict.message).filter(Boolean);
		return reasons.length > 0 ? `วางคาบไม่ได้: ${reasons.join(' · ')}` : 'ช่องนี้วางคาบไม่ได้';
	}
	if (!preview.mutation) return 'ไม่พบคำสั่งสำหรับวางคาบในตำแหน่งนี้';
	return null;
}
