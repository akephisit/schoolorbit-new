import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const projectRoot = path.resolve(__dirname, '../..');
const repoRoot = path.resolve(projectRoot, '..');

async function readProjectFile(relativePath) {
	return readFile(path.join(projectRoot, relativePath), 'utf8');
}

async function readRepoFile(relativePath) {
	return readFile(path.join(repoRoot, relativePath), 'utf8');
}

test('sidebar navigation is grouped into persisted collapsible workflow sections', async () => {
	const sidebar = await readProjectFile('src/lib/components/layout/Sidebar.svelte');
	const navigation = await readProjectFile('src/lib/components/layout/sidebar-navigation.ts');
	const preferences = await readProjectFile('src/lib/stores/ui-preferences.ts');
	const rules = await readRepoFile('.rules');

	assert.match(sidebar, /from '\$lib\/components\/ui\/dropdown-menu'/);
	assert.match(sidebar, /from '\$lib\/components\/ui\/button'/);
	assert.match(sidebar, /buildSidebarNavigation/);
	assert.match(sidebar, /setSidebarGroupExpanded/);
	assert.match(sidebar, /DropdownMenu\.Content/);
	assert.match(sidebar, /sectionExpanded/);

	assert.match(navigation, /academic-foundation/);
	assert.match(navigation, /academic-curriculum/);
	assert.match(navigation, /academic-timetable/);
	assert.match(navigation, /\/staff\/academic\/periods/);
	assert.match(navigation, /\/staff\/academic\/timetable/);

	const timetableSection = /id:\s*'academic-timetable'[\s\S]*?paths:\s*\[([\s\S]*?)\]/.exec(
		navigation
	);
	assert.ok(timetableSection, 'academic timetable section should be explicitly defined');
	assert.doesNotMatch(
		timetableSection[1],
		/\/staff\/academic\/periods/,
		'period settings should stay in academic foundation, not become timetable-only navigation'
	);

	assert.match(preferences, /sidebarExpandedGroups/);
	assert.match(preferences, /setSidebarGroupExpanded/);

	assert.match(rules, /Sidebar Navigation IA/);
	assert.match(rules, /collapsible workflow sections/);
});

test('collapsed sidebar renders a workspace icon rail with section flyouts', async () => {
	const sidebar = await readProjectFile('src/lib/components/layout/Sidebar.svelte');
	const navigation = await readProjectFile('src/lib/components/layout/sidebar-navigation.ts');
	const rules = await readRepoFile('.rules');

	assert.match(navigation, /WORKSPACE_ICONS/);
	assert.match(navigation, /icon:\s*WORKSPACE_ICONS\[section\.workspaceCode\]/);

	assert.match(sidebar, /function workspaceHasActiveItem/);
	assert.match(sidebar, /function collapsedWorkspaceTriggerClass/);
	assert.match(sidebar, /WorkspaceIcon = getIconComponent\(workspace\.icon\)/);
	assert.match(sidebar, /aria-label=\{workspace\.name\}/);
	assert.match(sidebar, /DropdownMenu\.Label[\s\S]*\{workspace\.name\}/);
	assert.match(sidebar, /\{#each workspace\.sections as section, sectionIndex \(section\.id\)\}/);
	assert.match(sidebar, /\{#each section\.items as item \(item\.id\)\}/);
	assert.doesNotMatch(sidebar, /collapsedSectionTriggerClass/);

	assert.match(rules, /workspace icon rail/);
});

test('collapsed sidebar keeps the rail vertical during width transition', async () => {
	const sidebar = await readProjectFile('src/lib/components/layout/Sidebar.svelte');

	assert.match(
		sidebar,
		/class=\{cn\(\s*'flex-1 overflow-y-auto overflow-x-hidden py-4 sidebar-nav',\s*isCollapsed\s*\?\s*'flex flex-col items-center gap-1 px-4'\s*:\s*'space-y-1 px-4'\s*\)\}/,
		'nav should become a flex column as soon as collapsed mode renders'
	);
	assert.match(
		sidebar,
		/buttonVariants\(\{ variant: 'ghost', size: 'icon' \}\),\s*'relative flex h-10 w-10 rounded-lg'/,
		'collapsed workspace triggers should override inline-flex with block-level flex layout'
	);
	assert.match(
		sidebar,
		/isCollapsed \? 'mx-auto w-10' : 'w-full justify-start'/,
		'work shortcut should stay centered in the collapsed rail during the transition'
	);
});
