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

test('sidebar navigation follows the persisted management hierarchy', async () => {
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

	assert.match(navigation, /id:\s*group\.code/);
	assert.match(navigation, /name:\s*group\.workspaceName/);
	assert.match(navigation, /icon:\s*group\.workspaceIcon\s*\|\|\s*'PanelLeft'/);
	assert.match(navigation, /order:\s*group\.workspaceOrder/);
	assert.match(navigation, /order:\s*group\.displayOrder/);
	assert.doesNotMatch(navigation, /SIDEBAR_SECTION_DEFINITIONS|WORKSPACE_LABELS|definitionsByPath/);
	assert.doesNotMatch(navigation, /\/staff\/academic/);

	assert.match(preferences, /sidebarExpandedGroups/);
	assert.match(preferences, /setSidebarGroupExpanded/);

	assert.match(rules, /Sidebar Navigation IA/);
	assert.match(rules, /management workspace → department\/work section → service link/);
	assert.match(rules, /Navigation placement never grants access/);
});

test('collapsed sidebar renders a workspace icon rail with section flyouts', async () => {
	const sidebar = await readProjectFile('src/lib/components/layout/Sidebar.svelte');
	const navigation = await readProjectFile('src/lib/components/layout/sidebar-navigation.ts');
	const rules = await readRepoFile('.rules');

	assert.match(navigation, /group\.workspaceIcon/);
	assert.match(navigation, /group\.workspaceName/);

	assert.match(sidebar, /function workspaceHasActiveItem/);
	assert.match(sidebar, /function collapsedWorkspaceTriggerClass/);
	assert.match(sidebar, /WorkspaceIcon = getIconComponent\(workspace\.icon\)/);
	assert.match(sidebar, /aria-label=\{workspace\.name\}/);
	assert.match(sidebar, /DropdownMenu\.Label[\s\S]*\{workspace\.name\}/);
	assert.match(sidebar, /\{#each workspace\.sections as section, sectionIndex \(section\.id\)\}/);
	assert.match(sidebar, /\{#each section\.items as item \(item\.id\)\}/);
	assert.doesNotMatch(sidebar, /collapsedSectionTriggerClass/);

	assert.match(rules, /management-workspace icon rail/);
});

test('personal home and menu administration share the configurable hierarchy', async () => {
	const dashboard = await readProjectFile('src/routes/(app)/staff/+page.svelte');
	const menuAdmin = await readProjectFile('src/routes/(app)/staff/menu/+page.svelte');
	const menuAdminApi = await readProjectFile('src/lib/api/menu-admin.ts');

	assert.match(dashboard, /getUserMenu/);
	assert.match(dashboard, /buildSidebarNavigation/);
	assert.match(dashboard, /serviceWorkspaces/);
	assert.match(dashboard, /หน้าหลักของฉัน/);
	assert.match(dashboard, /workStore\.fetchCounts/);

	assert.match(menuAdmin, /listMenuWorkspaces/);
	assert.match(menuAdmin, /WorkspaceManagementDialog/);
	assert.match(menuAdmin, /MenuItemManagementDialog/);
	assert.match(menuAdmin, /workspace_code/);
	assert.match(menuAdmin, /กำหนดกลุ่มบริหาร ฝ่าย\/งาน และตำแหน่งเมนู/);

	assert.match(menuAdminApi, /Schemas\['MenuWorkspace'\]/);
	assert.match(menuAdminApi, /\/api\/admin\/menu\/workspaces/);
	assert.doesNotMatch(menuAdminApi, /export interface CreateMenuGroupRequest/);
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
