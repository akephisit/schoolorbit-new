import type {
	AcademicContextOptionsResponse,
	AcademicTermOption,
	AcademicYearOption
} from '$lib/api/academic-context';
import type { Readable } from 'svelte/store';

export type { AcademicContextOptionsResponse, AcademicTermOption, AcademicYearOption };

export type AcademicContextRequirement =
	| 'none'
	| 'year_required'
	| 'term_required'
	| 'term_optional';

export type SelectedAcademicContext = {
	academicYearId: string | null;
	academicTermId: string | null;
};

export type AcademicContextState = {
	requirement: AcademicContextRequirement;
	options: AcademicContextOptionsResponse | null;
	selected: SelectedAcademicContext;
	status: 'hidden' | 'loading' | 'ready' | 'unavailable' | 'error';
};

export type AcademicContextResolution = Pick<
	AcademicContextState,
	'requirement' | 'selected' | 'status'
> & {
	replaceUrl: URL | null;
};

export type AcademicContextNavigationOptions = {
	replaceState?: boolean;
	noScroll?: boolean;
	keepFocus?: boolean;
};

export type AcademicContextNavigate = (
	url: URL,
	options: AcademicContextNavigationOptions
) => Promise<void>;

export type AcademicContextStore = Readable<AcademicContextState> & {
	sync: (routeId: string | null, url: URL) => Promise<void>;
	selectYear: (academicYearId: string) => Promise<void>;
	selectTerm: (academicTermId: string | null) => Promise<void>;
	retry: () => Promise<void>;
	reset: () => void;
};
