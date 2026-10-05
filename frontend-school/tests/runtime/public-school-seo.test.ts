import assert from 'node:assert/strict';
import test from 'node:test';
import {
	buildSchoolHomeSeo,
	getSchoolPublicIndexing,
	schoolRobots,
	schoolSitemap
} from '../../src/lib/school-public/seo.ts';

const backend = 'https://school-api.schoolorbit.app';
test('production schools use their own HTTPS root without queries or fragments', () => {
	for (const tenant of ['alpha', 'beta']) {
		const site = getSchoolPublicIndexing(
			new URL(`https://${tenant}.schoolorbit.app/?utm_source=test#statistics`),
			backend
		);
		assert.equal(site.indexable, true);
		assert.equal(site.homeUrl, `https://${tenant}.schoolorbit.app/`);
		assert.equal(site.sitemapUrl, `https://${tenant}.schoolorbit.app/sitemap.xml`);
		assert.match(
			schoolRobots(site),
			new RegExp(`Sitemap: https://${tenant}\\.schoolorbit\\.app/sitemap\\.xml`)
		);
		assert.equal((schoolSitemap(site).match(/<url>/g) || []).length, 1);
		assert(schoolSitemap(site).includes(`<loc>${site.homeUrl}</loc>`));
		assert(!schoolSitemap(site).includes('lastmod'));
	}
});
test('configured API domain determines the tenant domain rather than a baked production URL', () => {
	const site = getSchoolPublicIndexing(
		new URL('http://school.example.edu/?q=test'),
		'https://school-api.example.edu'
	);
	assert.equal(site.indexable, true);
	assert.equal(site.homeUrl, 'https://school.example.edu/');
});
test('sandbox, local, preview, reserved and unrelated hosts cannot be indexed', () => {
	for (const origin of [
		'https://sandbox.schoolorbit.app',
		'http://localhost:5173',
		'http://127.0.0.1:5173',
		'https://school.workers.dev',
		'https://preview.school.schoolorbit.app',
		'https://admin.schoolorbit.app',
		'https://school-api.schoolorbit.app',
		'https://www.schoolorbit.app',
		'https://schoolorbit.app',
		'https://alpha.schoolorbit.app.evil.test',
		'https://other.example.com'
	]) {
		const site = getSchoolPublicIndexing(new URL(origin), backend);
		assert.equal(site.indexable, false, origin);
		assert.equal(site.sitemapUrl, null);
		assert(!schoolRobots(site).includes('Sitemap:'));
		assert(!schoolSitemap(site).includes('<url>'));
	}
	assert.equal(
		getSchoolPublicIndexing(new URL('https://school.schoolorbit.app'), 'http://127.0.0.1:3000')
			.indexable,
		false
	);
});
test('metadata and structured data preserve names safely without publishing extra fields', () => {
	const site = getSchoolPublicIndexing(new URL('https://alpha.schoolorbit.app/'), backend);
	const name = 'โรงเรียน "ทดสอบ" & </script><script>alert(1)</script>\u2028';
	const seo = buildSchoolHomeSeo(
		{ schoolName: name, logoFileId: 'public-logo' },
		site,
		`${backend}/api/public/files/public-logo/content`
	);
	assert.equal(seo.title, `${name.trim()} — ข้อมูลและบริการสาธารณะ`);
	assert(seo.description.includes(name.trim()));
	assert(seo.structuredData);
	assert(!/[<>&\u2028\u2029]/.test(seo.structuredData));
	assert.deepEqual(JSON.parse(seo.structuredData), {
		'@context': 'https://schema.org',
		'@type': 'School',
		name: name.trim(),
		url: site.homeUrl,
		logo: seo.image
	});
});
test('missing branding uses truthful fallback metadata and omits unknown names and logos', () => {
	const site = getSchoolPublicIndexing(new URL('https://alpha.schoolorbit.app/'), backend);
	assert.equal(buildSchoolHomeSeo(null, site, null).structuredData, null);
	assert.equal(buildSchoolHomeSeo({ schoolName: '   ' }, site, null).siteName, 'เว็บไซต์โรงเรียน');
	const seo = buildSchoolHomeSeo({ schoolName: 'โรงเรียนทดสอบ' }, site, null);
	assert.equal(seo.image, null);
	assert(seo.structuredData);
	assert(!('logo' in JSON.parse(seo.structuredData)));
});
