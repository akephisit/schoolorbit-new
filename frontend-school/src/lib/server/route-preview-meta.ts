const SITE_NAME = 'SchoolOrbit';
const DEFAULT_DESCRIPTION = 'ระบบจัดการโรงเรียนแบบครบวงจร';
const DEFAULT_IMAGE_PATH = '/icon-192.png';

export const ROUTE_PREVIEW_MARKER = '<!-- schoolorbit-route-preview-meta -->';

interface PageRouteMeta {
	menu?: {
		title?: string;
	};
	preview?: {
		title?: string;
		description?: string;
		image?: string;
	};
}

interface PageRouteModule {
	_meta?: PageRouteMeta;
}

export interface RoutePreviewMeta {
	routePath: string;
	title: string;
	description: string;
	image: string;
}

interface RoutePreviewEntry extends RoutePreviewMeta {
	matcher: RegExp;
	specificity: number;
}

const routeModules = import.meta.glob('/src/routes/(app)/**/+page.ts', { eager: true }) as Record<
	string,
	PageRouteModule
>;

const routePreviewEntries = buildRoutePreviewEntries(routeModules);

export function getRoutePreviewMeta(pathname: string): RoutePreviewMeta | null {
	const normalizedPath = normalizePathname(pathname);
	const entry = routePreviewEntries.find((candidate) => candidate.matcher.test(normalizedPath));

	if (!entry) return null;

	return {
		routePath: entry.routePath,
		title: entry.title,
		description: entry.description,
		image: entry.image
	};
}

export function injectRoutePreviewMeta(
	html: string,
	meta: RoutePreviewMeta,
	url: URL
): string {
	if (!html.includes(ROUTE_PREVIEW_MARKER) && !html.includes('</head>')) {
		return html;
	}

	const cleanedHtml = removeManagedHeadTags(html);
	const tags = buildRoutePreviewTags(meta, url);

	if (cleanedHtml.includes(ROUTE_PREVIEW_MARKER)) {
		return cleanedHtml.replace(ROUTE_PREVIEW_MARKER, tags);
	}

	return cleanedHtml.replace('</head>', `${tags}\n</head>`);
}

function buildRoutePreviewEntries(modules: Record<string, PageRouteModule>): RoutePreviewEntry[] {
	return Object.entries(modules)
		.map(([filePath, pageModule]) => {
			const _meta = pageModule._meta;
			const title = _meta?.preview?.title ?? _meta?.menu?.title;

			if (!title) return null;

			const routePath = filePathToRoutePath(filePath);

			return {
				routePath,
				title,
				description: _meta?.preview?.description ?? DEFAULT_DESCRIPTION,
				image: _meta?.preview?.image ?? DEFAULT_IMAGE_PATH,
				matcher: routePathToMatcher(routePath),
				specificity: routePathSpecificity(routePath)
			};
		})
		.filter((entry): entry is RoutePreviewEntry => Boolean(entry))
		.sort((a, b) => b.specificity - a.specificity || b.routePath.length - a.routePath.length);
}

export function filePathToRoutePath(filePath: string): string {
	let routePath = filePath.replace(/\\/g, '/');
	const routeRootIndex = routePath.indexOf('/src/routes/');

	if (routeRootIndex >= 0) {
		routePath = routePath.slice(routeRootIndex + '/src/routes'.length);
	} else {
		routePath = routePath.replace(/^src\/routes/, '');
	}

	routePath = routePath.replace(/\/\+page\.ts$/, '');
	routePath = routePath.replace(/\/\([^/]+\)/g, '');

	return normalizePathname(routePath || '/');
}

export function routePathToMatcher(routePath: string): RegExp {
	const normalizedPath = normalizePathname(routePath);

	if (normalizedPath === '/') {
		return /^\/?$/;
	}

	const segments = normalizedPath.split('/').filter(Boolean);
	const pattern = segments.map(routeSegmentToPattern).join('/');

	return new RegExp(`^/${pattern}/?$`);
}

function routeSegmentToPattern(segment: string): string {
	if (/^\[\[\.\.\.[^\]]+\]\]$/.test(segment)) return '.*';
	if (/^\[\.\.\.[^\]]+\]$/.test(segment)) return '.+';
	if (/^\[\[[^\]]+\]\]$/.test(segment)) return '[^/]*';
	if (/^\[[^\]]+\]$/.test(segment)) return '[^/]+';

	return escapeRegExp(segment);
}

function routePathSpecificity(routePath: string): number {
	const segments = normalizePathname(routePath).split('/').filter(Boolean);
	const staticSegments = segments.filter((segment) => !segment.startsWith('[')).length;
	const dynamicSegments = segments.length - staticSegments;

	return staticSegments * 1000 + segments.length * 10 - dynamicSegments;
}

function buildRoutePreviewTags(meta: RoutePreviewMeta, url: URL): string {
	const title = `${meta.title} - ${SITE_NAME}`;
	const imageUrl = absoluteUrl(meta.image, url);

	return [
		`<title>${escapeHtmlText(title)}</title>`,
		`<meta data-schoolorbit-route-preview property="og:type" content="website" />`,
		`<meta data-schoolorbit-route-preview property="og:site_name" content="${SITE_NAME}" />`,
		`<meta data-schoolorbit-route-preview property="og:locale" content="th_TH" />`,
		`<meta data-schoolorbit-route-preview property="og:url" content="${escapeHtmlAttribute(url.href)}" />`,
		`<meta data-schoolorbit-route-preview property="og:title" content="${escapeHtmlAttribute(title)}" />`,
		`<meta data-schoolorbit-route-preview property="og:description" content="${escapeHtmlAttribute(meta.description)}" />`,
		`<meta data-schoolorbit-route-preview property="og:image" content="${escapeHtmlAttribute(imageUrl)}" />`,
		`<meta data-schoolorbit-route-preview name="twitter:card" content="summary" />`,
		`<meta data-schoolorbit-route-preview name="twitter:title" content="${escapeHtmlAttribute(title)}" />`,
		`<meta data-schoolorbit-route-preview name="twitter:description" content="${escapeHtmlAttribute(meta.description)}" />`,
		`<meta data-schoolorbit-route-preview name="twitter:image" content="${escapeHtmlAttribute(imageUrl)}" />`
	].join('\n\t');
}

function removeManagedHeadTags(html: string): string {
	return html
		.replace(/<title\b[^>]*>[\s\S]*?<\/title>\s*/gi, '')
		.replace(/<meta\b[^>]*data-schoolorbit-route-preview[^>]*>\s*/gi, '')
		.replace(
			/<meta\b(?=[^>]*(?:property|name)=["'](?:og:type|og:site_name|og:locale|og:url|og:title|og:description|og:image|twitter:card|twitter:title|twitter:description|twitter:image)["'])[^>]*>\s*/gi,
			''
		);
}

function normalizePathname(pathname: string): string {
	const pathnameOnly = pathname.split(/[?#]/, 1)[0] || '/';
	const withLeadingSlash = pathnameOnly.startsWith('/') ? pathnameOnly : `/${pathnameOnly}`;

	if (withLeadingSlash === '/') return '/';

	return withLeadingSlash.replace(/\/+$/, '');
}

function absoluteUrl(pathOrUrl: string, url: URL): string {
	return new URL(pathOrUrl, url.origin).href;
}

function escapeRegExp(value: string): string {
	return value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

function escapeHtmlText(value: string): string {
	return value.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
}

function escapeHtmlAttribute(value: string): string {
	return escapeHtmlText(value).replace(/"/g, '&quot;');
}
