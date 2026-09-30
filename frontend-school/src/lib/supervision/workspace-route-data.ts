import type {
	SupervisionCycle,
	SupervisionTemplateSummary,
	SupervisionTeacherStatusRow,
	SupervisionObservation
} from '$lib/api/supervision';
import type { RouteLoadResult } from '$lib/navigation/route-load';

export type WorkspaceSection =
	'overview' | 'cycles' | 'templates' | 'mine' | 'requests' | 'evaluate' | 'approvals';
export type ObservationRegion = { readable: boolean; items: SupervisionObservation[] };
export type TeacherStatusRegion = { cycleId: string; items: SupervisionTeacherStatusRow[] };
export type SupervisionWorkspaceRouteData = {
	section: WorkspaceSection;
	academicYearId: string | null;
	academicTermId: string | null;
	cycleId: string;
	cycles: Promise<RouteLoadResult<SupervisionCycle[]>> | null;
	templates: Promise<RouteLoadResult<SupervisionTemplateSummary[]>> | null;
	observations: Promise<RouteLoadResult<ObservationRegion>> | null;
	teacherStatus: Promise<RouteLoadResult<TeacherStatusRegion>> | null;
};
