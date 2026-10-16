export interface PublicSite {
	homeUrl: string;
	canonicalUrl: string;
	baseDomain: string;
	indexable: boolean;
}

export function getPublicSite(url: URL, api: string): PublicSite {
	let base: string | null = null;
	try {
		const backend = new URL(api);
		if (
			backend.protocol === 'https:' &&
			!backend.port &&
			backend.hostname.startsWith('admin-api.')
		) {
			base = backend.hostname.slice('admin-api.'.length);
		}
	} catch {
		/* Local previews have no production indexing identity. */
	}
	const origin = base ? `https://${base}` : url.origin;
	const canonical = new URL(origin);
	canonical.pathname = url.pathname;
	const after = url.searchParams.get('after');
	if (url.pathname === '/schools' && after && validSchoolSubdomain(after))
		canonical.searchParams.set('after', after);
	return {
		homeUrl: new URL('/', origin).href,
		canonicalUrl: canonical.href,
		baseDomain: base ?? url.hostname,
		indexable:
			!!base &&
			url.protocol === 'https:' &&
			url.hostname === base &&
			!url.port &&
			['/', '/schools'].includes(url.pathname)
	};
}

export function validSchoolSubdomain(value: string): boolean {
	return /^[a-z0-9](?:[a-z0-9-]{0,48}[a-z0-9])?$/.test(value);
}

export function canonicalRedirect(url: URL, api: string): URL | null {
	const site = getPublicSite(url, api);
	const home = new URL(site.homeUrl);
	if (home.protocol !== 'https:' || (site.baseDomain === url.hostname && url.protocol === 'https:'))
		return null;
	if (url.hostname !== `www.${site.baseDomain}` && url.hostname !== site.baseDomain) return null;
	const target = new URL(home);
	target.pathname = url.pathname;
	target.search = url.search;
	return target.href === url.href ? null : target;
}

export function publicRobots(site: PublicSite): string {
	return `User-agent: *\nDisallow:\n${site.indexable ? `Sitemap: ${new URL('/sitemap.xml', site.homeUrl).href}\n` : ''}`;
}

export function publicSitemap(site: PublicSite): string {
	const escape = (value: string) =>
		value.replace(
			/[<>&'"]/g,
			(c) => ({ '<': '&lt;', '>': '&gt;', '&': '&amp;', "'": '&apos;', '"': '&quot;' })[c] ?? c
		);
	const paths = site.indexable ? ['/', '/schools'] : [];
	return `<?xml version="1.0" encoding="UTF-8"?>\n<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">${paths.map((path) => `<url><loc>${escape(new URL(path, site.homeUrl).href)}</loc></url>`).join('')}</urlset>\n`;
}

export function serializeStructuredData(value: Record<string, unknown>): string {
	return JSON.stringify(value).replace(
		/[<>&\u2028\u2029]/g,
		(c) => `\\u${c.charCodeAt(0).toString(16).padStart(4, '0')}`
	);
}
