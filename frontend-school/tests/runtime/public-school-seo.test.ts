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
	assert.equal(seo.title, `${name.trim()} - ระบบจัดการโรงเรียนครบวงจร`);
	assert(seo.description.includes(name.trim()));
	assert(seo.structuredData);
	assert(!/[<>&\u2028\u2029]/.test(seo.structuredData));
	assert.deepEqual(JSON.parse(seo.structuredData), {
		'@context': 'https://schema.org',
		'@type': 'School',
		name: name.trim(),
		url: site.homeUrl,
		logo: `${backend}/api/public/files/public-logo/content`
	});
	assert.equal(new URL(seo.image).pathname, '/og-image.png');
	assert.notEqual(JSON.parse(seo.structuredData).logo, seo.image);
});
test('missing school branding uses the platform logo and never invents a school name', () => {
	const site = getSchoolPublicIndexing(new URL('https://alpha.schoolorbit.app/'), backend);
	assert.equal(buildSchoolHomeSeo(null, site, null).structuredData, null);
	assert.equal(buildSchoolHomeSeo({ schoolName: '   ' }, site, null).siteName, 'เว็บไซต์โรงเรียน');
	const seo = buildSchoolHomeSeo({ schoolName: 'โรงเรียนทดสอบ' }, site, null);
	assert.equal(new URL(seo.image).origin, 'https://alpha.schoolorbit.app');
	assert(seo.structuredData);
	assert.equal(
		JSON.parse(seo.structuredData).logo,
		'https://alpha.schoolorbit.app/schoolorbit.png'
	);
});

test('homepage copy preserves the actual name and adds the school prefix only once', () => {
	const site = getSchoolPublicIndexing(new URL('https://alpha.schoolorbit.app/'), backend);
	for (const name of ['ซับน้อยเหนือวิทยาคม', 'โรงเรียนซับน้อยเหนือวิทยาคม']) {
		const seo = buildSchoolHomeSeo({ schoolName: name }, site, null);
		assert.equal(seo.title, `${name} - ระบบจัดการโรงเรียนครบวงจร`);
		assert.equal(
			seo.description,
			'โรงเรียนซับน้อยเหนือวิทยาคม เชื่อมครู นักเรียน และผู้ปกครอง ผ่านระบบจัดการโรงเรียนครบวงจร'
		);
	}
	assert.equal(
		buildSchoolHomeSeo({ schoolName: 'โรงเรียน ซับน้อยเหนือวิทยาคม' }, site, null).description,
		'โรงเรียน ซับน้อยเหนือวิทยาคม เชื่อมครู นักเรียน และผู้ปกครอง ผ่านระบบจัดการโรงเรียนครบวงจร'
	);
	const missing = buildSchoolHomeSeo(null, site, null);
	assert.equal(missing.title, 'เว็บไซต์โรงเรียน - ระบบจัดการโรงเรียนครบวงจร');
	assert.equal(
		missing.description,
		'เว็บไซต์โรงเรียน เชื่อมครู นักเรียน และผู้ปกครอง ผ่านระบบจัดการโรงเรียนครบวงจร'
	);
});

test('school card URLs belong to the current origin and change with public branding', () => {
	const info = { schoolName: 'ซับน้อยเหนือวิทยาคม', logoFileId: 'crest-one' };
	const site = getSchoolPublicIndexing(new URL('https://alpha.schoolorbit.app/'), backend);
	const image = buildSchoolHomeSeo(info, site, null).image;
	assert.match(image, /^https:\/\/alpha\.schoolorbit\.app\/og-image\.png\?v=3-[a-f0-9]+$/);
	assert.equal(
		buildSchoolHomeSeo({ ...info, schoolName: ` ${info.schoolName} ` }, site, null).image,
		image
	);
	assert.notEqual(buildSchoolHomeSeo({ ...info, schoolName: 'ชื่อใหม่' }, site, null).image, image);
	assert.notEqual(
		buildSchoolHomeSeo({ ...info, logoFileId: 'crest-two' }, site, null).image,
		image
	);
	const other = getSchoolPublicIndexing(new URL('https://beta.schoolorbit.app/'), backend);
	assert.equal(
		new URL(buildSchoolHomeSeo(info, other, null).image).origin,
		'https://beta.schoolorbit.app'
	);
	assert(!image.includes(info.schoolName));
	assert(!image.includes(info.logoFileId));
});
