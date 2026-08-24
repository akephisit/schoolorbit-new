import type { TimetableEntry, TimetablePeriodSummary } from '$lib/api/timetable';
import type { GeneratePdfOptions, TimetablePage } from '$lib/utils/pdf';

export interface StaffOwnTimetablePdfInput {
	teacherName: string;
	termName?: string | null;
	termCode?: string | null;
	academicYearName?: string | null;
	entries: TimetableEntry[];
	dayValues: string[];
	periods: TimetablePeriodSummary[];
}

export interface StaffOwnTimetablePdfDownload {
	fileName: string;
	pages: TimetablePage[];
}

export interface StaffOwnTimetablePdfReadiness {
	loading: boolean;
	isExporting: boolean;
	selectedYearId: string;
	selectedAcademicTermId: string;
	selectedTermYearId?: string;
	loadedSelectionKey: string;
	entryCount: number;
	periodCount: number;
}

export interface StaffOwnTimetablePdfDependencies {
	generatePdf: (
		pages: TimetablePage[],
		fileName?: string,
		options?: GeneratePdfOptions
	) => Promise<void>;
	setExporting: (value: boolean) => void;
	onSuccess: () => void;
	onError: (error: unknown) => void;
}

export function staffOwnTimetableSelectionKey(yearId: string, academicTermId: string): string {
	return yearId && academicTermId ? `${yearId}:${academicTermId}` : '';
}

export function canDownloadStaffOwnTimetablePdf(state: StaffOwnTimetablePdfReadiness): boolean {
	const selectedSelectionKey = staffOwnTimetableSelectionKey(
		state.selectedYearId,
		state.selectedAcademicTermId
	);

	return Boolean(
		!state.loading &&
		!state.isExporting &&
		selectedSelectionKey &&
		state.selectedTermYearId === state.selectedYearId &&
		state.loadedSelectionKey === selectedSelectionKey &&
		state.entryCount > 0 &&
		state.periodCount > 0
	);
}

export async function runStaffOwnTimetablePdfDownload(
	download: StaffOwnTimetablePdfDownload,
	dependencies: StaffOwnTimetablePdfDependencies
): Promise<void> {
	dependencies.setExporting(true);
	try {
		await dependencies.generatePdf(download.pages, download.fileName, { layout: 'full' });
		dependencies.onSuccess();
	} catch (error: unknown) {
		dependencies.onError(error);
	} finally {
		dependencies.setExporting(false);
	}
}

export function buildStaffOwnTimetablePdfDownload(
	input: StaffOwnTimetablePdfInput
): StaffOwnTimetablePdfDownload {
	const normalizedTeacherName = input.teacherName.trim();
	const teacherLabel = normalizedTeacherName
		? normalizedTeacherName.startsWith('ครู')
			? normalizedTeacherName
			: `ครู${normalizedTeacherName}`
		: 'ครู';
	const termName = input.termName?.trim();
	const termCode = input.termCode?.trim();
	const termLabel = termName || (termCode ? `ภาคเรียนที่ ${termCode}` : 'ภาคเรียน');
	const academicYearLabel = input.academicYearName?.trim() || 'ปีการศึกษา';
	const subTitle = `${termLabel} ${academicYearLabel}`;
	const title = `ตารางสอน ${teacherLabel}`;

	return {
		fileName: `${title} ${subTitle}`.replaceAll('/', '-'),
		pages: [
			{
				title,
				subTitle,
				dayValues: input.dayValues,
				periods: input.periods.map((period, orderIndex) => ({
					id: period.id,
					order_index: orderIndex,
					name: period.name,
					start_time: period.startTime ?? '',
					end_time: period.endTime ?? ''
				})),
				timetableEntries: input.entries,
				viewMode: 'INSTRUCTOR'
			}
		]
	};
}
