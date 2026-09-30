import type {
	SupervisionCycle,
	SupervisionTemplateSummary,
	SupervisionTeacherStatusRow
} from '$lib/api/supervision';
import type { RouteLoadResult } from '$lib/navigation/route-load';

export type ManagementSection = 'overview' | 'cycles' | 'templates';
export type TeacherStatusRegion = { cycleId: string; items: SupervisionTeacherStatusRow[] };
export type SupervisionManagementRouteData = {
	section: ManagementSection;
	academicYearId: string | null;
	academicTermId: string | null;
	cycleId: string;
	cycles: Promise<RouteLoadResult<SupervisionCycle[]>> | null;
	templates: Promise<RouteLoadResult<SupervisionTemplateSummary[]>> | null;
	teacherStatus: Promise<RouteLoadResult<TeacherStatusRegion>> | null;
};
