import assert from 'node:assert/strict';
import test from 'node:test';
import { readFile } from 'node:fs/promises';
const source = (path) => readFile(new URL(`../../src/${path}`, import.meta.url), 'utf8');
test('menu loader schedules independent visible catalogs with exact read permission', async () => {
	const loader = await source('routes/(app)/staff/menu/+page.ts');
	for (const name of [
		'listMenuWorkspaces',
		'listMenuGroups',
		'listMenuItems',
		'MENU_READ_ALL',
		'school:app-identity'
	])
		assert.ok(loader.includes(name));
	assert.match(loader, /captureRouteLoad/);
	assert.doesNotMatch(loader, /Promise\.all|previewRecommended/);
	const page = await source('routes/(app)/staff/menu/+page.svelte');
	assert.doesNotMatch(page, /onMount|\bloadData\(/);
	assert.match(page, /workspaceState/);
	assert.match(page, /groupState/);
	assert.match(page, /itemState/);
});
test('menu catalog transport accepts cancellation and route fetch', async () => {
	const api = await source('lib/api/menu-admin.ts');
	for (const name of [
		'listMenuWorkspaces',
		'listMenuGroups',
		'listMenuItems',
		'previewRecommendedAcademicMenuTemplate'
	])
		assert.match(api, new RegExp(`${name}\\([\\s\\S]*?options: ApiRequestOptions`));
});
test('menu templates and editors reject late draft effects', async () => {
	const template = await source('lib/components/menu/AcademicMenuTemplateDialog.svelte');
	assert.match(template, /LatestRequest/);
	assert.match(template, /onDestroy/);
	for (const name of [
		'GroupManagementDialog',
		'WorkspaceManagementDialog',
		'MenuItemManagementDialog'
	])
		assert.match(await source(`lib/components/menu/${name}.svelte`), /ownsDraft/);
});
