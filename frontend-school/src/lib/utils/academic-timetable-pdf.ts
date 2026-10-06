import { timetableTeacherLabel } from '../academic/timetable/teacher-label.ts';
import type { TimetableBlock, TimetableBlockWorkspace } from '#lib/api/timetable.js';
import type { TimetablePage } from '#lib/utils/pdf.js';
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
	selectedOwnerIds?: readonly string[]
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
		title: `${ownerView === 'teacher' ? 'ตารางสอน' : 'ตารางเรียน'} ${ownerView === 'teacher' ? timetableTeacherLabel(row.label) : row.label}`,
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
	return {
		pages,
		fileName:
			`${view === 'wholeSchool' ? 'ตารางเรียนทุกห้อง' : pages.length > 1 ? (ownerView === 'teacher' ? 'ตารางสอนครูที่เลือก' : 'ตารางเรียนที่เลือก') : (pages[0]?.title ?? 'ตารางสอน')} ${context} ${versionLabel}`
				.replaceAll('/', '-')
				.replaceAll('\\', '-')
	};
}
