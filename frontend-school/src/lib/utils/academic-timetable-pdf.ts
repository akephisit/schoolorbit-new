import { timetableTeacherFullLabel } from '../academic/timetable/teacher-label.ts';
import type { TimetableBlock, TimetableBlockWorkspace } from '#lib/api/timetable.js';
import type { GeneratePdfOptions, TimetablePage } from '#lib/utils/pdf.js';
import {
	blockBelongsToRow,
	createTimetableBoardState,
	rowsForTimetableView,
	type TimetableBoardView,
	type TimetablePageView
} from '#lib/academic/timetable/board-state.js';

function blocksForOwner(
	blocks: TimetableBlock[],
	view: TimetableBoardView,
	ownerId: string
): TimetableBlock[] {
	return blocks
		.filter((block) => block.isActive && blockBelongsToRow(block, view, ownerId))
		.map((block) => ({
			...block,
			groups: block.groups.filter((group) =>
				view === 'homeroom'
					? group.homeroomIds.includes(ownerId)
					: view === 'learning_group'
						? group.learningGroupId === ownerId
						: group.instructors.some((teacher) => teacher.teacherId === ownerId)
			),
			homerooms: block.homerooms.filter(
				(target) => target.isActive && (view !== 'homeroom' || target.homeroomId === ownerId)
			),
			teachers: block.teachers.filter((target) => target.isActive)
		}));
}

export function buildAcademicTimetablePdfDownload(
	workspace: TimetableBlockWorkspace,
	view: TimetablePageView,
	ownerId: string | null,
	termName: string,
	yearName: string,
	selectedOwnerIds?: readonly string[],
	layout: NonNullable<GeneratePdfOptions['layout']> = 'full'
): { pages: TimetablePage[]; fileName: string } {
	const ownerView = view === 'wholeSchool' ? 'homeroom' : view;
	const rows = rowsForTimetableView(createTimetableBoardState(workspace), ownerView).filter(
		(row) =>
			selectedOwnerIds
				? selectedOwnerIds.includes(row.id)
				: view === 'wholeSchool' || row.id === ownerId
	);
	const periods = workspace.bellPeriods
		.filter((period) => period.isActive)
		.sort((a, b) => a.orderIndex - b.orderIndex);
	const dayValues = [
		...new Set([
			'MON',
			'TUE',
			'WED',
			'THU',
			'FRI',
			...periods.flatMap((period) => (period.applicableDays ?? '').split(',').filter(Boolean)),
			...workspace.blocks.filter((block) => block.isActive).map((block) => block.dayOfWeek)
		])
	];
	const context = `${termName || 'ภาคเรียน'} ${yearName || 'ปีการศึกษา'}`;
	const versionLabel = workspace.version.status === 'draft' ? 'แบบร่าง' : 'เผยแพร่แล้ว';
	const subTitle = context;
	const roomNames = Object.fromEntries(workspace.rooms.map((room) => [room.id, room.name]));
	const homeroomNames = Object.fromEntries(workspace.homerooms.map((room) => [room.id, room.name]));
	const pages: TimetablePage[] = rows.map((row) => ({
		title: `${ownerView === 'teacher' ? 'ตารางสอน' : 'ตารางเรียน'} ${ownerView === 'teacher' ? timetableTeacherFullLabel(row.label) : row.label}`,
		subTitle,
		dayValues,
		periods: periods.map((period) => ({
			id: period.id,
			order_index: period.orderIndex,
			name: period.name,
			start_time: period.startTime,
			end_time: period.endTime
		})),
		timetableBlocks: blocksForOwner(workspace.blocks, ownerView, row.id),
		viewMode: ownerView === 'teacher' ? 'INSTRUCTOR' : 'CLASSROOM',
		roomNames,
		homeroomNames
	}));
	// A compact selection key distinguishes batches without putting every name in the filename.
	let selectionKey = 2166136261;
	for (const char of [workspace.version.id, ownerView, ...rows.map((row) => row.id)].join(':')) {
		selectionKey = Math.imul(selectionKey ^ char.charCodeAt(0), 16777619) >>> 0;
	}
	const batchLabel =
		ownerView === 'teacher'
			? 'ตารางสอนครูที่เลือก'
			: ownerView === 'learning_group'
				? 'ตารางเรียนกลุ่มที่เลือก'
				: 'ตารางเรียนห้องที่เลือก';
	const fileTitle =
		pages.length > 1
			? `${batchLabel} ${pages.length} (${selectionKey.toString(16)})`
			: (pages[0]?.title ?? 'ตารางสอน');
	const layoutLabel = layout === 'portrait-2col' ? '6ต่อหน้า' : '1ต่อหน้า';
	return {
		pages,
		fileName: `${fileTitle} ${context} ${layoutLabel} ${versionLabel}`
			.replaceAll('/', '-')
			.replaceAll('\\', '-')
	};
}
