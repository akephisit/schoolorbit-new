import {
	listAcademicContextOptions,
	listPublicAcademicContextOptions
} from '#lib/api/academic-context.js';
import { createContext } from 'svelte';
import { derived, get, writable } from 'svelte/store';
import {
	getAcademicContextOptionsSource,
	getAcademicContextRequirement,
	resolveAcademicContextUrl
} from './route-context';
import type {
	AcademicContextNavigate,
	AcademicContextOptionsResponse,
	AcademicContextOptionsSource,
	AcademicContextState,
	AcademicContextStore
} from './types';

type ContextState = Omit<AcademicContextState, 'options'> & {
	source: AcademicContextOptionsSource;
};

const initialState = (): ContextState => ({
	requirement: 'none',
	source: 'staff',
	optionsBySource: { staff: null, published: null },
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
	loadPublishedOptions?: () => Promise<AcademicContextOptionsResponse>;
};

export function createAcademicContextStore({
	navigate,
	loadOptions = listAcademicContextOptions,
	loadPublishedOptions = listPublicAcademicContextOptions
}: AcademicContextStoreDependencies): AcademicContextStore {
	const state = writable(initialState());
	const view = derived(state, ({ source, ...current }): AcademicContextState => ({
		...current,
		options: current.optionsBySource[source]
	}));
	const sources: Record<
		AcademicContextOptionsSource,
		{
			request: Promise<AcademicContextOptionsResponse> | null;
			revision: number;
			load: () => Promise<AcademicContextOptionsResponse>;
		}
	> = {
		staff: { request: null, revision: 0, load: loadOptions },
		published: { request: null, revision: 0, load: loadPublishedOptions }
	};
	let latestRouteId: string | null = null;
	let latestUrl: URL | null = null;
	let revision = 0;
	let sessionGeneration = 0;
	type RepairNavigation = {
		generation: number;
		sourceUrl: string;
		targetUrl: string;
		promise: Promise<void>;
	};
	let repairNavigation: RepairNavigation | null = null;

	function repairUrl(source: URL, target: URL): Promise<void> {
		if (
			repairNavigation?.generation === sessionGeneration &&
			repairNavigation.sourceUrl === source.href &&
			repairNavigation.targetUrl === target.href
		)
			return repairNavigation.promise;
		// A same-identity refresh can sync the original URL while goto is still pending.
		// Share that navigation only; each sync revision still owns its publication.
		const operation: RepairNavigation = {
			generation: sessionGeneration,
			sourceUrl: source.href,
			targetUrl: target.href,
			promise: Promise.resolve()
				.then(() => {
					if (operation.generation !== sessionGeneration || repairNavigation !== operation) return;
					return navigate(target, { replace: true, reset: false });
				})
				.finally(() => {
					if (repairNavigation === operation) repairNavigation = null;
				})
		};
		repairNavigation = operation;
		return operation.promise;
	}

	function optionsForSession(
		source: AcademicContextOptionsSource
	): Promise<AcademicContextOptionsResponse> {
		const options = sources[source];
		if (options.request) return options.request;
		const currentGeneration = sessionGeneration;
		const sourceRevision = options.revision;
		const request = options.load().then(
			(response) => {
				if (currentGeneration !== sessionGeneration) return response;
				if (sourceRevision !== options.revision) return optionsForSession(source);
				state.update((current) => ({
					...current,
					optionsBySource: { ...current.optionsBySource, [source]: response }
				}));
				return response;
			},
			(error: unknown) => {
				if (currentGeneration === sessionGeneration && sourceRevision !== options.revision)
					return optionsForSession(source);
				if (options.request === request) options.request = null;
				throw error;
			}
		);
		options.request = request;
		return request;
	}

	function invalidateOptions(source: AcademicContextOptionsSource): number {
		const options = sources[source];
		options.request = null;
		return ++options.revision;
	}

	function replaceOptions(options: AcademicContextOptionsResponse): void {
		invalidateOptions('published');
		sources.staff.revision += 1;
		sources.staff.request = Promise.resolve(options);
		// School settings never navigate an existing form or replace its explicit context.
		state.update((current) => ({
			...current,
			optionsBySource: { staff: options, published: null }
		}));
	}

	async function refreshOptions(): Promise<boolean> {
		const currentGeneration = sessionGeneration;
		invalidateOptions('published');
		state.update((current) => ({
			...current,
			optionsBySource: { ...current.optionsBySource, published: null }
		}));
		const refreshRevision = invalidateOptions('staff');
		try {
			await optionsForSession('staff');
			return currentGeneration === sessionGeneration && refreshRevision === sources.staff.revision;
		} catch {
			// The mutation already succeeded. Keep usable discovery and retry the read on
			// the next route sync instead of reporting an ambiguous save failure.
			return false;
		}
	}

	async function primeOptions(): Promise<void> {
		try {
			await optionsForSession('staff');
		} catch {
			// An unrelated route stays hidden; a later prime or route sync retries the read.
		}
	}

	async function sync(routeId: string | null, url: URL): Promise<void> {
		const currentRevision = ++revision;
		if (repairNavigation && repairNavigation.sourceUrl !== url.href) repairNavigation = null;
		latestRouteId = routeId;
		latestUrl = new URL(url);
		const requirement = getAcademicContextRequirement(routeId);
		const source = getAcademicContextOptionsSource(routeId);

		if (requirement === 'none') {
			state.update((current) => ({ ...current, requirement, source, status: 'hidden' }));
			return;
		}

		state.update((current) => ({
			...current,
			requirement,
			source,
			selected: current.source === source ? current.selected : initialState().selected,
			status: 'loading'
		}));

		let options: AcademicContextOptionsResponse;
		try {
			options = await optionsForSession(source);
		} catch {
			if (currentRevision !== revision) return;
			state.update((current) => ({
				...current,
				optionsBySource: { ...current.optionsBySource, [source]: null },
				selected: { academicYearId: null, academicTermId: null },
				status: 'error'
			}));
			return;
		}

		if (currentRevision !== revision) return;
		const resolved = resolveAcademicContextUrl(requirement, options, url);
		if (resolved.replaceUrl) {
			latestUrl = new URL(resolved.replaceUrl);
			try {
				await repairUrl(url, resolved.replaceUrl);
			} catch {
				if (currentRevision !== revision) return;
				state.update((current) => ({
					...current,
					selected: resolved.selected,
					status: 'error'
				}));
				return;
			}
		}

		if (currentRevision !== revision) return;
		state.update((current) => ({
			...current,
			selected: resolved.selected,
			status: resolved.status
		}));
	}

	async function selectYear(academicYearId: string): Promise<void> {
		const current = get(view);
		if (!latestUrl || !current.options) return;
		if (!current.options.years.some((year) => year.id === academicYearId)) return;

		const nextUrl = new URL(latestUrl);
		nextUrl.searchParams.set('academicYearId', academicYearId);
		const currentTermId = nextUrl.searchParams.get('academicTermId');
		const currentTerm = current.options.terms.find((term) => term.id === currentTermId);
		if (!currentTerm || currentTerm.academicYearId !== academicYearId) {
			nextUrl.searchParams.delete('academicTermId');
		}

		await navigate(nextUrl, { reset: false });
	}

	async function selectTerm(academicTermId: string | null): Promise<void> {
		const current = get(view);
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

		await navigate(nextUrl, { reset: false });
	}

	async function retry(): Promise<void> {
		invalidateOptions(get(state).source);
		if (latestUrl) await sync(latestRouteId, latestUrl);
	}

	function reset(): void {
		repairNavigation = null;
		revision += 1;
		sessionGeneration += 1;
		invalidateOptions('staff');
		invalidateOptions('published');
		latestRouteId = null;
		latestUrl = null;
		clearAcademicContextDirtySources();
		state.set(initialState());
	}

	return {
		subscribe: view.subscribe,
		primeOptions,
		refreshOptions,
		replaceOptions,
		sync,
		selectYear,
		selectTerm,
		retry,
		reset
	};
}

export const [getAcademicContextStore, setAcademicContextStore] =
	createContext<AcademicContextStore>();
