import type { MenuGroup, MenuItem } from '$lib/api/menu';

export type SidebarMenuItem = MenuItem & {
	groupCode: string;
	groupName: string;
	workspaceCode: string;
};

export type SidebarMenuSection = {
	id: string;
	name: string;
	icon: string;
	workspaceCode: string;
	order: number;
	defaultOpen: boolean;
	items: SidebarMenuItem[];
};

export type SidebarWorkspaceSection = {
	code: string;
	name: string;
	icon: string;
	order: number;
	sections: SidebarMenuSection[];
};

export function buildSidebarNavigation(menuGroups: MenuGroup[]): SidebarWorkspaceSection[] {
	const workspaceMap = new Map<string, SidebarWorkspaceSection>();

	for (const group of menuGroups) {
		const workspaceCode = group.workspaceCode;
		const section = {
			id: group.code,
			name: group.name,
			icon: group.icon || 'Circle',
			workspaceCode,
			order: group.displayOrder,
			defaultOpen: workspaceCode === 'home',
			items: group.items.map((item) => ({
				...item,
				groupCode: group.code,
				groupName: group.name,
				workspaceCode
			}))
		} satisfies SidebarMenuSection;

		const workspace =
			workspaceMap.get(workspaceCode) ??
			({
				code: workspaceCode,
				name: group.workspaceName,
				icon: group.workspaceIcon || 'PanelLeft',
				order: group.workspaceOrder,
				sections: []
			} satisfies SidebarWorkspaceSection);

		workspace.sections.push(section);
		workspaceMap.set(workspaceCode, workspace);
	}

	return Array.from(workspaceMap.values())
		.map((workspace) => ({
			...workspace,
			sections: workspace.sections.sort((a, b) => a.order - b.order || a.name.localeCompare(b.name))
		}))
		.sort((a, b) => a.order - b.order || a.name.localeCompare(b.name));
}
