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
	image: string | null;
	structuredData: string | null;
}

export function buildSchoolHomeSeo(
	info: PublicSchoolInfo | null,
	site: SchoolPublicIndexing,
	logoUrl: string | null
): SchoolHomeSeo {
	const name = info?.schoolName?.trim() || null;
	const image = info?.logoFileId ? logoUrl : null;
	return {
		title: `${name || 'เว็บไซต์โรงเรียน'} — ข้อมูลและบริการสาธารณะ`,
		description: name
			? `ข้อมูลโรงเรียน${name} สถิตินักเรียน ครู ห้องเรียน โครงสร้างบริหาร และบริการสาธารณะของโรงเรียน`
			: 'ข้อมูลโรงเรียน สถิตินักเรียน ครู ห้องเรียน โครงสร้างบริหาร และบริการสาธารณะ',
		siteName: name || 'เว็บไซต์โรงเรียน',
		image,
		structuredData: name
			? JSON.stringify({
					'@context': 'https://schema.org',
					'@type': 'School',
					name,
					url: site.homeUrl,
					...(image ? { logo: image } : {})
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
