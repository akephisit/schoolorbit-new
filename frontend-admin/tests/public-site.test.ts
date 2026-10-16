import assert from 'node:assert/strict';
import test from 'node:test';
import {
	canonicalRedirect,
	getPublicSite,
	publicRobots,
	publicSitemap,
	serializeStructuredData
} from '../src/lib/public-site.ts';

const api = 'https://admin-api.example.test';

test('both www schemes redirect permanently to the configured apex without losing paths or queries', () => {
	for (const scheme of ['http', 'https']) {
		const target = canonicalRedirect(
			new URL(`${scheme}://www.example.test/schools?after=alpha&utm_source=test`),
			api
		);
		assert.equal(target?.href, 'https://example.test/schools?after=alpha&utm_source=test');
	}
	assert.equal(
		canonicalRedirect(new URL('http://example.test/'), api)?.href,
		'https://example.test/'
	);
	for (const path of ['//evil.test/path', '/%2F%2Fevil.test/path']) {
		assert.equal(
			canonicalRedirect(new URL('https://www.example.test' + path), api)?.hostname,
			'example.test'
		);
		assert.equal(
			new URL(getPublicSite(new URL('https://example.test' + path), api).canonicalUrl).hostname,
			'example.test'
		);
	}
	for (const host of [
		'example.test',
		'admin.example.test',
		'alpha.example.test',
		'www.example.test.evil.test',
		'preview.workers.dev'
	]) {
		assert.equal(canonicalRedirect(new URL(`https://${host}/`), api), null);
	}
});

test('only public apex pages are indexable and canonical URLs omit tracking and account tokens', () => {
	const home = getPublicSite(
		new URL('https://example.test/?utm_source=mail&token=private#section'),
		api
	);
	assert.equal(home.indexable, true);
	assert.equal(home.canonicalUrl, 'https://example.test/');
	const directory = getPublicSite(
		new URL('https://example.test/schools?after=alpha&utm_source=test'),
		api
	);
	assert.equal(directory.indexable, true);
	assert.equal(directory.canonicalUrl, 'https://example.test/schools?after=alpha');
	for (const path of [
		'/login',
		'/register',
		'/verify-email?token=private',
		'/reset-password?token=private',
		'/dashboard',
		'/unknown'
	]) {
		assert.equal(getPublicSite(new URL(path, home.homeUrl), api).indexable, false);
	}
	for (const origin of [
		'https://www.example.test',
		'https://sandbox.example.test',
		'https://preview.workers.dev',
		'http://localhost:5173',
		'https://example.test.evil.test'
	]) {
		const site = getPublicSite(new URL(origin), api);
		assert.equal(site.indexable, false);
		assert(!publicSitemap(site).includes('<url>'));
		assert(!publicRobots(site).includes('Sitemap:'));
	}
	assert.equal(getPublicSite(new URL('http://localhost:5173'), 'not-a-url').indexable, false);
});

test('the sitemap is independent of account/database reads and contains only canonical public pages', () => {
	const site = getPublicSite(new URL('https://example.test/?tracking=ignored'), api);
	assert.equal(
		publicRobots(site),
		'User-agent: *\nDisallow:\nSitemap: https://example.test/sitemap.xml\n'
	);
	const sitemap = publicSitemap(site);
	assert.match(sitemap, /<loc>https:\/\/example\.test\/<\/loc>/);
	assert.match(sitemap, /<loc>https:\/\/example\.test\/schools<\/loc>/);
	assert.equal((sitemap.match(/<url>/g) || []).length, 2);
	assert.doesNotMatch(sitemap, /login|dashboard|token|lastmod|<loc>https:\/\/www\./);
});

test('school names cannot escape structured data script boundaries', () => {
	const value = {
		'@type': 'ListItem',
		name: 'โรงเรียน </script><script>alert(1)</script> & \u2028'
	};
	const serialized = serializeStructuredData(value);
	assert.doesNotMatch(serialized, /[<>&\u2028\u2029]/);
	assert.deepEqual(JSON.parse(serialized), value);
});
