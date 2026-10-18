import type { PublicSchoolInfo } from '#lib/api/school.js';

export interface SchoolPublicIndexing {
	indexable: boolean;
	homeUrl: string;
	sitemapUrl: string | null;
}

/** The configured school-api hostname owns the production tenant domain. */
export function getSchoolPublicIndexing(url: URL, backendUrl: string): SchoolPublicIndexing {
	const backend = new URL(backendUrl);
	const baseDomain = backend.hostname.startsWith('school-api.')
		? backend.hostname.slice('school-api.'.length)
		: null;
	const suffix = baseDomain ? `.${baseDomain}` : null;
	const tenant =
		suffix && url.hostname.endsWith(suffix) ? url.hostname.slice(0, -suffix.length) : '';
	const productionHost =
		backend.protocol === 'https:' &&
		!url.port &&
		/^[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?$/.test(tenant);
	const reserved = ['sandbox', 'www', 'admin', 'school-api', 'admin-api', 'server'];
	const indexable = productionHost && !reserved.includes(tenant);
	const home = new URL('/', url);
	if (productionHost) home.protocol = 'https:';
	return {
		indexable,
		homeUrl: home.href,
		sitemapUrl: indexable ? new URL('/sitemap.xml', home).href : null
	};
}

export interface SchoolHomeSeo {
	title: string;
	description: string;
	siteName: string;
	image: string;
	imageAlt: string;
	structuredData: string | null;
}

/** Public branding only; bump the template version when the card design changes. */
function schoolShareImageUrl(info: PublicSchoolInfo | null, homeUrl: string): string {
	const branding = `v1:${info?.schoolName?.trim() || ''}:${info?.logoFileId || ''}`;
	let hash = 2166136261;
	for (let index = 0; index < branding.length; index++) {
		hash = Math.imul(hash ^ branding.charCodeAt(index), 16777619);
	}
	const image = new URL('/og-image.png', homeUrl);
	image.searchParams.set('v', `1-${(hash >>> 0).toString(16)}`);
	return image.href;
}

export function buildSchoolHomeSeo(
	info: PublicSchoolInfo | null,
	site: SchoolPublicIndexing,
	logoUrl: string | null
): SchoolHomeSeo {
	const name = info?.schoolName?.trim() || null;
	const siteName = name || 'เว็บไซต์โรงเรียน';
	const schoolName = name?.replace(/^โรงเรียน/, '') || null;
	const logo = logoUrl || new URL('/schoolorbit.png', site.homeUrl).href;
	return {
		title: `${siteName} - ระบบจัดการโรงเรียนครบวงจร`,
		description: `${schoolName ? `โรงเรียน${schoolName}` : 'เว็บไซต์โรงเรียน'} เชื่อมครู นักเรียน และผู้ปกครอง ผ่านระบบจัดการโรงเรียนครบวงจร`,
		siteName,
		image: schoolShareImageUrl(info, site.homeUrl),
		imageAlt: `${siteName} - ระบบจัดการโรงเรียนครบวงจร`,
		structuredData: name
			? JSON.stringify({
					'@context': 'https://schema.org',
					'@type': 'School',
					name,
					url: site.homeUrl,
					logo
				}).replace(
					/[<>&\u2028\u2029]/g,
					(character) => `\\u${character.charCodeAt(0).toString(16).padStart(4, '0')}`
				)
			: null
	};
}

export function schoolRobots(site: SchoolPublicIndexing): string {
	return `User-agent: *\nDisallow:\n${site.sitemapUrl ? `Sitemap: ${site.sitemapUrl}\n` : ''}`;
}

export function schoolSitemap(site: SchoolPublicIndexing): string {
	const location = site.homeUrl.replace(/[<>&'"]/g, (character) => {
		const entities: Record<string, string> = {
			'<': '&lt;',
			'>': '&gt;',
			'&': '&amp;',
			"'": '&apos;',
			'"': '&quot;'
		};
		return entities[character];
	});
	return `<?xml version="1.0" encoding="UTF-8"?>\n<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">${site.indexable ? `<url><loc>${location}</loc></url>` : ''}</urlset>\n`;
}
