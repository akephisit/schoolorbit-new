import type {
	AcademicContextOptionsResponse,
	AcademicContextRequirement,
	AcademicContextResolution,
	AcademicContextState,
	SelectedAcademicContext
} from './types';

type RouteMetaModule = {
	_meta?: {
		academicContext?: unknown;
	};
};

type RouteModuleMap = Record<string, RouteMetaModule>;
export type AcademicContextRouteResolver = (routeId: string | null) => AcademicContextRequirement;

const requirements = new Set<AcademicContextRequirement>([
	'none',
	'year_required',
	'term_required',
	'term_optional'
]);

function isAcademicContextRequirement(value: unknown): value is AcademicContextRequirement {
	return typeof value === 'string' && requirements.has(value as AcademicContextRequirement);
}

function routeIdFromFilePath(filePath: string): string {
	return filePath.replace('/src/routes', '').replace('/+page.ts', '');
}

function isStaffRoute(routeId: string): boolean {
	return routeId === '/(app)/staff' || routeId.startsWith('/(app)/staff/');
}

export function createAcademicContextRouteResolver(
	routeModules: RouteModuleMap
): AcademicContextRouteResolver {
	const requirementByRouteId = new Map<string, AcademicContextRequirement>();

	for (const [filePath, module] of Object.entries(routeModules)) {
		const declaredRequirement = module._meta?.academicContext;
		if (declaredRequirement === undefined) continue;
		if (!isAcademicContextRequirement(declaredRequirement)) {
			throw new Error(
				`Invalid academic context requirement in ${filePath}: ${String(declaredRequirement)}`
			);
		}

		const routeId = routeIdFromFilePath(filePath);
		if (isStaffRoute(routeId)) {
			requirementByRouteId.set(routeId, declaredRequirement);
		}
	}

	return (routeId: string | null): AcademicContextRequirement => {
		if (!routeId || !isStaffRoute(routeId)) return 'none';

		let currentRouteId = routeId;
		while (currentRouteId.length > 0) {
			const requirement = requirementByRouteId.get(currentRouteId);
			if (requirement) return requirement;

			const lastSlash = currentRouteId.lastIndexOf('/');
			if (lastSlash <= 0) break;
			currentRouteId = currentRouteId.slice(0, lastSlash);
		}

		return 'none';
	};
}

function discoverRouteModules(): RouteModuleMap {
	return import.meta.glob('/src/routes/[(]app[)]/**/+page.ts', {
		eager: true
	}) as RouteModuleMap;
}

let defaultRouteResolver: AcademicContextRouteResolver | null = null;

export function getAcademicContextRequirement(routeId: string | null): AcademicContextRequirement {
	defaultRouteResolver ??= createAcademicContextRouteResolver(discoverRouteModules());
	return defaultRouteResolver(routeId);
}

export function readAcademicContextFromUrl(url: URL): SelectedAcademicContext {
	return {
		academicYearId: url.searchParams.get('academicYearId'),
		academicTermId: url.searchParams.get('academicTermId')
	};
}

export function academicContextualMenuPath(
	path: string,
	selected: SelectedAcademicContext,
	options: AcademicContextOptionsResponse | null,
	resolveRequirement: AcademicContextRouteResolver = getAcademicContextRequirement
): string {
	const target = new URL(path, 'https://schoolorbit.invalid');
	const requirement = resolveRequirement(`/(app)${target.pathname}`);
	if (requirement === 'none') return path;

	const selectedYearIsValid =
		!options || options.years.some((year) => year.id === selected.academicYearId);
	const defaultYearIsValid = options?.years.some(
		(year) => year.id === options.defaultAcademicYearId
	);
	const academicYearId = selectedYearIsValid
		? selected.academicYearId
		: defaultYearIsValid
			? (options?.defaultAcademicYearId ?? null)
			: null;
	const selectedTerm = options?.terms.find((term) => term.id === selected.academicTermId);
	const defaultTerm = options?.terms.find((term) => term.id === options.defaultAcademicTermId);
	const academicTermId =
		selected.academicTermId && (!options || selectedTerm?.academicYearId === academicYearId)
			? selected.academicTermId
			: requirement === 'term_required' && defaultTerm?.academicYearId === academicYearId
				? defaultTerm.id
				: null;

	target.searchParams.delete('academicYearId');
	target.searchParams.delete('academicTermId');
	if (academicYearId) {
		target.searchParams.set('academicYearId', academicYearId);
	}
	if (
		academicYearId &&
		academicTermId &&
		(requirement === 'term_required' || requirement === 'term_optional')
	) {
		target.searchParams.set('academicTermId', academicTermId);
	}
	return `${target.pathname}${target.search}${target.hash}`;
}

export function shouldHoldAcademicContextPage(
	requirement: AcademicContextRequirement,
	url: URL,
	status: AcademicContextState['status']
): boolean {
	if (requirement === 'none' || status === 'unavailable' || status === 'error') return false;

	const selected = readAcademicContextFromUrl(url);
	if (!selected.academicYearId) return true;
	return requirement === 'term_required' && !selected.academicTermId;
}

function resolution(
	requirement: AcademicContextRequirement,
	status: AcademicContextResolution['status'],
	selected: SelectedAcademicContext,
	replaceUrl: URL | null = null
): AcademicContextResolution {
	return { requirement, status, selected, replaceUrl };
}

export function resolveAcademicContextUrl(
	requirement: AcademicContextRequirement,
	options: AcademicContextOptionsResponse,
	url: URL
): AcademicContextResolution {
	const emptySelection = { academicYearId: null, academicTermId: null };
	if (requirement === 'none') {
		return resolution(requirement, 'hidden', emptySelection);
	}

	const fromUrl = readAcademicContextFromUrl(url);
	let academicYearId = fromUrl.academicYearId;
	let replaceUrl: URL | null = null;

	if (academicYearId === null) {
		const defaultYearId = options.defaultAcademicYearId ?? null;
		if (!defaultYearId || !options.years.some((year) => year.id === defaultYearId)) {
			return resolution(requirement, 'unavailable', emptySelection);
		}
		academicYearId = defaultYearId;
		replaceUrl = new URL(url);
		replaceUrl.searchParams.set('academicYearId', defaultYearId);
	}

	if (!options.years.some((year) => year.id === academicYearId)) {
		return resolution(requirement, 'unavailable', {
			academicYearId,
			academicTermId: fromUrl.academicTermId
		});
	}

	if (requirement === 'year_required') {
		return resolution(requirement, 'ready', { academicYearId, academicTermId: null }, replaceUrl);
	}

	let academicTermId = fromUrl.academicTermId;
	if (academicTermId === null && requirement === 'term_optional') {
		return resolution(requirement, 'ready', { academicYearId, academicTermId: null }, replaceUrl);
	}

	if (academicTermId === null) {
		const defaultTermId = options.defaultAcademicTermId ?? null;
		const defaultTerm = options.terms.find((term) => term.id === defaultTermId);
		if (!defaultTerm || defaultTerm.academicYearId !== academicYearId) {
			return resolution(
				requirement,
				'unavailable',
				{ academicYearId, academicTermId: null },
				replaceUrl
			);
		}
		academicTermId = defaultTerm.id;
		replaceUrl ??= new URL(url);
		replaceUrl.searchParams.set('academicTermId', academicTermId);
	}

	const term = options.terms.find((candidate) => candidate.id === academicTermId);
	if (!term || term.academicYearId !== academicYearId) {
		return resolution(requirement, 'unavailable', {
			academicYearId,
			academicTermId
		});
	}

	return resolution(requirement, 'ready', { academicYearId, academicTermId }, replaceUrl);
}
