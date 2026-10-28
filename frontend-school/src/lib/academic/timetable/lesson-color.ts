import type {
	TimetableBlock,
	TimetableBlockPlacementSource,
	TimetableBlockWorkspace
} from '#lib/api/timetable.js';

export type TimetableLessonColorKey = `group:${string}` | `offering:${string}`;
export interface TimetableLessonTone {
	light: string;
	dark: string;
}
export type TimetableLessonColors = ReadonlyMap<TimetableLessonColorKey, TimetableLessonTone>;

// Paired categorical fills vary lightness and chroma as well as hue. Neutral text keeps its contrast.
const palette = [
	[10, 0.86, 0.12, 0.33, 0.1],
	[25, 0.94, 0.06, 0.43, 0.09],
	[40, 0.82, 0.11, 0.28, 0.08],
	[55, 0.9, 0.11, 0.38, 0.1],
	[70, 0.95, 0.09, 0.3, 0.08],
	[85, 0.84, 0.14, 0.42, 0.1],
	[100, 0.91, 0.13, 0.35, 0.1],
	[115, 0.96, 0.08, 0.28, 0.07],
	[130, 0.83, 0.13, 0.42, 0.1],
	[145, 0.9, 0.11, 0.33, 0.09],
	[160, 0.95, 0.07, 0.43, 0.08],
	[175, 0.83, 0.11, 0.28, 0.07],
	[190, 0.91, 0.09, 0.38, 0.08],
	[205, 0.95, 0.06, 0.3, 0.07],
	[220, 0.84, 0.11, 0.43, 0.09],
	[235, 0.91, 0.08, 0.35, 0.09],
	[250, 0.96, 0.04, 0.28, 0.07],
	[265, 0.83, 0.11, 0.42, 0.11],
	[280, 0.9, 0.09, 0.33, 0.1],
	[295, 0.95, 0.05, 0.43, 0.1],
	[310, 0.83, 0.12, 0.28, 0.09],
	[325, 0.91, 0.08, 0.38, 0.1],
	[340, 0.95, 0.06, 0.3, 0.09],
	[355, 0.83, 0.12, 0.43, 0.11]
] as const;

function lab(lightness: number, chroma: number, hue: number): number[] {
	return [
		lightness,
		chroma * Math.cos((hue * Math.PI) / 180),
		chroma * Math.sin((hue * Math.PI) / 180)
	];
}
const tones: TimetableLessonTone[] = palette.map(([hue, light, lightChroma, dark, darkChroma]) => ({
	light: `oklch(${light} ${lightChroma} ${hue})`,
	dark: `oklch(${dark} ${darkChroma} ${hue})`
}));
const distances = palette.map((left) =>
	palette.map((right) =>
		Math.min(
			Math.hypot(
				...lab(left[1], left[2], left[0]).map(
					(value, index) => value - lab(right[1], right[2], right[0])[index]
				)
			),
			Math.hypot(
				...lab(left[3], left[4], left[0]).map(
					(value, index) => value - lab(right[3], right[4], right[0])[index]
				)
			)
		)
	)
);

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

function identityHash(key: TimetableLessonColorKey): number {
	let hash = 2166136261;
	for (const character of key) {
		hash = Math.imul(hash ^ character.charCodeAt(0), 16777619);
	}
	// Mix all bits so consecutive group UUIDs do not produce nearly identical hues.
	hash = Math.imul(hash ^ (hash >>> 16), 0x85ebca6b);
	hash = Math.imul(hash ^ (hash >>> 13), 0xc2b2ae35);
	hash = (hash ^ (hash >>> 16)) >>> 0;
	return hash;
}

/** Allocate once from complete source relationships, never the visible/pending lesson list. */
export function buildTimetableLessonColors(
	workspace: Pick<TimetableBlockWorkspace, 'learningGroups' | 'synchronizedDemands'>
): TimetableLessonColors {
	const resources = new Map<TimetableLessonColorKey, Set<string>>();
	const offeringTeachers = new Map<string, Set<string>>();
	const synchronizedOfferingIds = new Set(
		workspace.synchronizedDemands.map((demand) => demand.learningOfferingId)
	);
	for (const group of workspace.learningGroups) {
		if (!synchronizedOfferingIds.has(group.learningOfferingId))
			resources.set(
				ordinaryLessonColorKey(group.id),
				new Set([
					...group.homeroomIds.map((id) => `room:${id}`),
					...group.eligibleInstructors.map((teacher) => `teacher:${teacher.teacherId}`)
				])
			);
		const teachers = offeringTeachers.get(group.learningOfferingId) ?? new Set<string>();
		for (const teacher of group.eligibleInstructors) teachers.add(teacher.teacherId);
		offeringTeachers.set(group.learningOfferingId, teachers);
	}
	for (const demand of workspace.synchronizedDemands) {
		resources.set(
			synchronizedLessonColorKey(demand.learningOfferingId),
			new Set([
				...demand.intendedHomeroomIds.map((id) => `room:${id}`),
				...[...(offeringTeachers.get(demand.learningOfferingId) ?? [])].map((id) => `teacher:${id}`)
			])
		);
	}
	const memberships = new Map<string, number>();
	for (const targets of resources.values())
		for (const target of targets) memberships.set(target, (memberships.get(target) ?? 0) + 1);
	const degree = (key: TimetableLessonColorKey) =>
		[...resources.get(key)!].reduce((sum, target) => sum + (memberships.get(target) ?? 1) - 1, 0);
	const keys = [...resources.keys()].sort((a, b) => degree(b) - degree(a) || a.localeCompare(b));
	const usedByResource = new Map<string, number[]>();
	const usage = Array<number>(palette.length).fill(0);
	const result = new Map<TimetableLessonColorKey, TimetableLessonTone>();
	for (const key of keys) {
		const counts = Array<number>(palette.length).fill(0);
		for (const target of resources.get(key)!) {
			const used = usedByResource.get(target);
			if (used) for (let index = 0; index < counts.length; index++) counts[index] += used[index];
		}
		const neighbors = counts.flatMap((count, index) => (count ? [index] : []));
		const preference = identityHash(key) % palette.length;
		let chosen = 0,
			bestDistance = -1;
		for (let index = 0; index < palette.length; index++) {
			const distance = neighbors.length
				? Math.min(...neighbors.map((neighbor) => distances[index][neighbor]))
				: 1;
			if (
				counts[index] < counts[chosen] ||
				(counts[index] === counts[chosen] &&
					(distance > bestDistance ||
						(distance === bestDistance &&
							(usage[index] < usage[chosen] ||
								(usage[index] === usage[chosen] &&
									(index - preference + palette.length) % palette.length <
										(chosen - preference + palette.length) % palette.length)))))
			) {
				chosen = index;
				bestDistance = distance;
			}
		}
		result.set(key, tones[chosen]);
		usage[chosen]++;
		for (const target of resources.get(key)!) {
			const used = usedByResource.get(target) ?? Array<number>(palette.length).fill(0);
			used[chosen]++;
			usedByResource.set(target, used);
		}
	}
	return result;
}

export function timetableLessonColorStyle(
	key: TimetableLessonColorKey | null,
	colors: TimetableLessonColors
): string | undefined {
	const tone = key === null ? undefined : colors.get(key);
	return tone
		? `--timetable-lesson-light: ${tone.light}; --timetable-lesson-dark: ${tone.dark}`
		: undefined;
}
