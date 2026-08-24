import { listAcademicContextOptions } from '$lib/api/academic-context';
import { createContext } from 'svelte';
import { get, writable } from 'svelte/store';
import { getAcademicContextRequirement, resolveAcademicContextUrl } from './route-context';
import type {
	AcademicContextNavigate,
	AcademicContextOptionsResponse,
	AcademicContextState,
	AcademicContextStore
} from './types';

const initialState = (): AcademicContextState => ({
	requirement: 'none',
	options: null,
	selected: { academicYearId: null, academicTermId: null },
	status: 'hidden'
});

const dirtySources = new Map<string, () => boolean>();

export function registerAcademicContextDirtySource(
	key: string,
	isDirty: () => boolean
): () => void {
	if (!key.trim()) throw new Error('Academic context dirty-source key is required');
	dirtySources.set(key, isDirty);

	return () => {
		if (dirtySources.get(key) === isDirty) dirtySources.delete(key);
	};
}

export function hasAcademicContextDirtySource(): boolean {
	for (const isDirty of dirtySources.values()) {
		try {
			if (isDirty()) return true;
		} catch {
			return true;
		}
	}
	return false;
}

function clearAcademicContextDirtySources(): void {
	dirtySources.clear();
}

type AcademicContextStoreDependencies = {
	navigate: AcademicContextNavigate;
	loadOptions?: () => Promise<AcademicContextOptionsResponse>;
};

export function createAcademicContextStore({
	navigate,
	loadOptions = listAcademicContextOptions
}: AcademicContextStoreDependencies): AcademicContextStore {
	const state = writable<AcademicContextState>(initialState());
	let cachedOptions: Promise<AcademicContextOptionsResponse> | null = null;
	let latestRouteId: string | null = null;
	let latestUrl: URL | null = null;
	let revision = 0;

	function optionsForSession(): Promise<AcademicContextOptionsResponse> {
		cachedOptions ??= loadOptions().catch((error: unknown) => {
			cachedOptions = null;
			throw error;
		});
		return cachedOptions;
	}

	async function sync(routeId: string | null, url: URL): Promise<void> {
		const currentRevision = ++revision;
		latestRouteId = routeId;
		latestUrl = new URL(url);
		const requirement = getAcademicContextRequirement(routeId);

		if (requirement === 'none') {
			state.set(initialState());
			return;
		}

		const previous = get(state);
		state.set({
			requirement,
			options: previous.options,
			selected: previous.selected,
			status: 'loading'
		});

		let options: AcademicContextOptionsResponse;
		try {
			options = await optionsForSession();
		} catch {
			if (currentRevision !== revision) return;
			state.set({
				requirement,
				options: null,
				selected: { academicYearId: null, academicTermId: null },
				status: 'error'
			});
			return;
		}

		if (currentRevision !== revision) return;
		const resolved = resolveAcademicContextUrl(requirement, options, url);
		if (resolved.replaceUrl) {
			latestUrl = new URL(resolved.replaceUrl);
			try {
				await navigate(resolved.replaceUrl, {
					replaceState: true,
					noScroll: true,
					keepFocus: true
				});
			} catch {
				if (currentRevision !== revision) return;
				state.set({
					requirement,
					options,
					selected: resolved.selected,
					status: 'error'
				});
				return;
			}
		}

		if (currentRevision !== revision) return;
		state.set({
			requirement,
			options,
			selected: resolved.selected,
			status: resolved.status
		});
	}

	async function selectYear(academicYearId: string): Promise<void> {
		const current = get(state);
		if (!latestUrl || !current.options) return;
		if (!current.options.years.some((year) => year.id === academicYearId)) return;

		const nextUrl = new URL(latestUrl);
		nextUrl.searchParams.set('academicYearId', academicYearId);
		const currentTermId = nextUrl.searchParams.get('academicTermId');
		const currentTerm = current.options.terms.find((term) => term.id === currentTermId);
		if (!currentTerm || currentTerm.academicYearId !== academicYearId) {
			nextUrl.searchParams.delete('academicTermId');
		}

		await navigate(nextUrl, { noScroll: true, keepFocus: true });
	}

	async function selectTerm(academicTermId: string | null): Promise<void> {
		const current = get(state);
		if (!latestUrl || !current.options || !current.selected.academicYearId) return;

		const nextUrl = new URL(latestUrl);
		if (academicTermId === null) {
			if (current.requirement !== 'term_optional') return;
			nextUrl.searchParams.delete('academicTermId');
		} else {
			const term = current.options.terms.find((candidate) => candidate.id === academicTermId);
			if (!term || term.academicYearId !== current.selected.academicYearId) return;
			nextUrl.searchParams.set('academicTermId', academicTermId);
		}

		await navigate(nextUrl, { noScroll: true, keepFocus: true });
	}

	async function retry(): Promise<void> {
		cachedOptions = null;
		if (latestUrl) await sync(latestRouteId, latestUrl);
	}

	function reset(): void {
		revision += 1;
		cachedOptions = null;
		latestRouteId = null;
		latestUrl = null;
		clearAcademicContextDirtySources();
		state.set(initialState());
	}

	return {
		subscribe: state.subscribe,
		sync,
		selectYear,
		selectTerm,
		retry,
		reset
	};
}

export const [getAcademicContextStore, setAcademicContextStore] =
	createContext<AcademicContextStore>();
