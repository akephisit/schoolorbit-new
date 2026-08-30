export type HomeroomReadinessFilter = 'all' | 'ready' | 'attention';

type DeliveryRoomShape = {
	homeroom: { name: string };
	gradeLevel: { name: string };
	studyProgram: { code: string; name: string; curriculumName: string };
	expectedCount: number;
	readyCount: number;
	items?: ReadonlyArray<{ alignmentStates: readonly string[] }>;
	extraOfferings?: readonly unknown[];
};

function normalize(value: string): string {
	return value.trim().toLocaleLowerCase('th-TH');
}

function needsAttention(room: DeliveryRoomShape): boolean {
	return (
		room.expectedCount === 0 ||
		room.readyCount < room.expectedCount ||
		(room.extraOfferings?.length ?? 0) > 0 ||
		(room.items?.some((item) => !item.alignmentStates.includes('matches_curriculum')) ?? false)
	);
}

export function filterHomeroomDeliveryRooms<T extends DeliveryRoomShape>(
	rooms: readonly T[],
	search: string,
	readiness: HomeroomReadinessFilter
): T[] {
	const needle = normalize(search);
	return rooms.filter((room) => {
		const matchesReadiness =
			readiness === 'all' ||
			(readiness === 'ready' && !needsAttention(room)) ||
			(readiness === 'attention' && needsAttention(room));
		if (!matchesReadiness) return false;
		if (!needle) return true;
		return normalize(
			[
				room.homeroom.name,
				room.gradeLevel.name,
				room.studyProgram.code,
				room.studyProgram.name,
				room.studyProgram.curriculumName
			].join(' ')
		).includes(needle);
	});
}

export function summarizeHomeroomDelivery(rooms: readonly DeliveryRoomShape[]) {
	return rooms.reduce(
		(summary, room) => ({
			roomCount: summary.roomCount + 1,
			expectedCount: summary.expectedCount + room.expectedCount,
			readyCount: summary.readyCount + room.readyCount,
			attentionRoomCount:
				summary.attentionRoomCount +
				(needsAttention(room) ? 1 : 0)
		}),
		{ roomCount: 0, expectedCount: 0, readyCount: 0, attentionRoomCount: 0 }
	);
}
