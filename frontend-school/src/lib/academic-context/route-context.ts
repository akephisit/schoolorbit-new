import type {
	AcademicContextOptionsResponse,
	AcademicContextRequirement,
	AcademicContextResolution,
	SelectedAcademicContext
} from './types';

type RouteMetaModule = {
	_meta?: {
		academicContext?: unknown;
	};
};

type RouteModuleMap = Record<string, RouteMetaModule>;
type AcademicContextRouteResolver = (routeId: string | null) => AcademicContextRequirement;

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
		const activeYearId = options.activeAcademicYearId ?? null;
		if (!activeYearId || !options.years.some((year) => year.id === activeYearId)) {
			return resolution(requirement, 'unavailable', emptySelection);
		}
		academicYearId = activeYearId;
		replaceUrl = new URL(url);
		replaceUrl.searchParams.set('academicYearId', activeYearId);
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
		const activeTermId = options.activeAcademicTermId ?? null;
		const activeTerm = options.terms.find((term) => term.id === activeTermId);
		if (!activeTerm || activeTerm.academicYearId !== academicYearId) {
			return resolution(
				requirement,
				'unavailable',
				{ academicYearId, academicTermId: null },
				replaceUrl
			);
		}
		academicTermId = activeTerm.id;
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
