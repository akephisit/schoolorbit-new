import type { TimetableBlock, TimetableBlockPlacementSource } from '#lib/api/timetable.js';

export type TimetableLessonColorKey = `group:${string}` | `offering:${string}`;

export function ordinaryLessonColorKey(learningGroupId: string): TimetableLessonColorKey {
	return `group:${learningGroupId}`;
}

export function synchronizedLessonColorKey(learningOfferingId: string): TimetableLessonColorKey {
	return `offering:${learningOfferingId}`;
}

export function timetableBlockColorKey(block: TimetableBlock): TimetableLessonColorKey | null {
	if (block.blockKind === 'structural') return null;
	if (block.schedulingMode === 'synchronized') {
		return block.learningOfferingId ? synchronizedLessonColorKey(block.learningOfferingId) : null;
	}
	const groupId = block.groups.map((group) => group.learningGroupId).sort()[0];
	return groupId ? ordinaryLessonColorKey(groupId) : null;
}

export function timetablePlacementColorKey(
	source: TimetableBlockPlacementSource,
	existingBlock?: TimetableBlock
): TimetableLessonColorKey | null {
	if (source.kind === 'ordinary_demand') return ordinaryLessonColorKey(source.learningGroupId);
	if (source.kind === 'synchronized_offering') {
		return synchronizedLessonColorKey(source.learningOfferingId);
	}
	return existingBlock ? timetableBlockColorKey(existingBlock) : null;
}

/** A continuous hue keeps colors independent of the visible workspace and demand ordering. */
export function timetableLessonColorStyle(key: TimetableLessonColorKey | null): string | undefined {
	if (key === null) return undefined;
	let hash = 2166136261;
	for (const character of key) {
		hash = Math.imul(hash ^ character.charCodeAt(0), 16777619);
	}
	// Mix all bits so consecutive group UUIDs do not produce nearly identical hues.
	hash = Math.imul(hash ^ (hash >>> 16), 0x85ebca6b);
	hash = Math.imul(hash ^ (hash >>> 13), 0xc2b2ae35);
	hash = (hash ^ (hash >>> 16)) >>> 0;
	return `--timetable-lesson-hue: ${((hash / 4294967296) * 360).toFixed(2)}`;
}
