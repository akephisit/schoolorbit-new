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
		// Own-timetable responses carry group names without a homeroom directory.
		const shortNames = group.name.match(/(?:ม|ป|อ)\.\s*\d+\s*\/\s*\d+/g);
		return shortNames?.map((name) => name.replace(/\s/g, '')) ?? [group.name];
	});
	names.push(...block.homerooms.filter((room) => room.isActive).map((room) => room.name));
	return [...new Set(names.filter(Boolean))].join(', ');
}
