import type { TimetableBlock } from '#lib/api/timetable.js';

/** Student targets for teacher PDFs; special periods have no class line. */
export function timetablePdfClassLabel(
	block: TimetableBlock,
	homeroomNames: Record<string, string> = {}
): string {
	if (
		block.blockKind === 'structural' ||
		(block.blockKind === 'activity' && block.schedulingMode === 'synchronized')
	)
		return '';

	const names = block.groups.flatMap((group) => {
		const rooms = group.homeroomIds.map((id) => homeroomNames[id]).filter(Boolean);
		if (rooms.length) return rooms;
		return group.homeroomNames.map((name) => name.trim()).filter(Boolean);
	});
	names.push(...block.homerooms.filter((room) => room.isActive).map((room) => room.name));
	return [...new Set(names.filter(Boolean))].join(', ');
}
