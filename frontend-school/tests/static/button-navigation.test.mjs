import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';
import { compile } from 'svelte/compiler';
import { render } from 'svelte/server';

const source = await readFile(
	new URL('../../src/lib/components/ui/button/button.svelte', import.meta.url),
	'utf8'
);
const resolverUrl = `data:text/javascript,${encodeURIComponent(`
	export const calls = [];
	export function resolve(href) {
		calls.push(href);
		return '/app/' + href.replace(/^\\//, '');
	}
`)}`;
const imports = {
	'svelte/internal/server': import.meta.resolve('svelte/internal/server'),
	'#lib/utils.js': new URL('../../src/lib/utils.ts', import.meta.url).href,
	'tailwind-variants': import.meta.resolve('tailwind-variants'),
	'$app/paths': resolverUrl
};
const compiled = compile(source, { filename: 'button.svelte', generate: 'server' }).js.code.replace(
	/from '([^']+)'/g,
	(match, specifier) => (imports[specifier] ? `from ${JSON.stringify(imports[specifier])}` : match)
);
const { default: Button } = await import(
	`data:text/javascript;base64,${Buffer.from(compiled).toString('base64')}`
);
const { calls } = await import(resolverUrl);

test('Button keeps fragment links on the current page without route resolution', () => {
	calls.length = 0;
	const { body } = render(Button, { props: { href: '#academic-year-settings' } });
	assert.match(body, /href="#academic-year-settings"/);
	assert.deepEqual(calls, []);
});

test('Button continues to resolve ordinary application routes', () => {
	calls.length = 0;
	const { body } = render(Button, { props: { href: '/staff/academic/year-lifecycle' } });
	assert.match(body, /href="\/app\/staff\/academic\/year-lifecycle"/);
	assert.deepEqual(calls, ['/staff/academic/year-lifecycle']);
});

test('disabled Button links have no navigation target and retain accessible disabled state', () => {
	for (const href of ['#academic-year-settings', '/staff/academic/year-lifecycle']) {
		calls.length = 0;
		const { body } = render(Button, { props: { href, disabled: true } });
		assert.doesNotMatch(body, /\shref=/);
		assert.match(body, /aria-disabled="true"/);
		assert.match(body, /role="link"/);
		assert.match(body, /tabindex="-1"/);
		assert.deepEqual(calls, []);
	}
});
