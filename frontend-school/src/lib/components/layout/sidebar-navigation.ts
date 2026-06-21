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

type SidebarSectionDefinition = {
	id: string;
	name: string;
	icon: string;
	workspaceCode: string;
	order: number;
	defaultOpen?: boolean;
	paths: string[];
};

export const WORKSPACE_LABELS: Record<string, string> = {
	home: 'งานประจำ',
	academic: 'งานวิชาการ',
	student_affairs: 'กิจการนักเรียน',
	personnel: 'บุคลากร',
	operations: 'งานบริหารทั่วไป',
	settings: 'ตั้งค่าระบบ'
};

export const WORKSPACE_ORDER = [
	'home',
	'academic',
	'student_affairs',
	'personnel',
	'operations',
	'settings'
];

export const WORKSPACE_ICONS: Record<string, string> = {
	home: 'Inbox',
	academic: 'GraduationCap',
	student_affairs: 'UserRoundCheck',
	personnel: 'Users',
	operations: 'Building2',
	settings: 'Settings'
};

export const SIDEBAR_SECTION_DEFINITIONS: SidebarSectionDefinition[] = [
	{
		id: 'home-main',
		name: 'งานประจำ',
		icon: 'LayoutDashboard',
		workspaceCode: 'home',
		order: 10,
		defaultOpen: true,
		paths: [
			'/staff',
			'/staff/timetable',
			'/student',
			'/student/timetable',
			'/student/activities',
			'/parent'
		]
	},
	{
		id: 'academic-foundation',
		name: 'โครงสร้างวิชาการ',
		icon: 'Framer',
		workspaceCode: 'academic',
		order: 10,
		defaultOpen: true,
		paths: ['/staff/academic/structure', '/staff/academic/periods', '/staff/academic/classrooms']
	},
	{
		id: 'academic-curriculum',
		name: 'หลักสูตรและรายวิชา',
		icon: 'BookOpen',
		workspaceCode: 'academic',
		order: 20,
		defaultOpen: true,
		paths: [
			'/staff/academic/subject-groups',
			'/staff/academic/subjects',
			'/staff/academic/study-plans',
			'/staff/academic/planning',
			'/staff/academic/activities'
		]
	},
	{
		id: 'academic-students',
		name: 'นักเรียนและห้องเรียน',
		icon: 'Users',
		workspaceCode: 'academic',
		order: 30,
		paths: ['/staff/students', '/staff/academic/enrollments', '/staff/academic/supervision']
	},
	{
		id: 'academic-admission',
		name: 'รับสมัคร',
		icon: 'ClipboardList',
		workspaceCode: 'academic',
		order: 40,
		paths: ['/staff/academic/admission']
	},
	{
		id: 'academic-timetable',
		name: 'ตารางสอน',
		icon: 'CalendarDays',
		workspaceCode: 'academic',
		order: 50,
		paths: ['/staff/academic/timetable/today', '/staff/academic/timetable']
	},
	{
		id: 'personnel-management',
		name: 'บุคลากร',
		icon: 'Users',
		workspaceCode: 'personnel',
		order: 10,
		defaultOpen: true,
		paths: ['/staff/manage', '/staff/achievements']
	},
	{
		id: 'personnel-organization',
		name: 'โครงสร้างองค์กร',
		icon: 'Building2',
		workspaceCode: 'personnel',
		order: 20,
		paths: ['/staff/organization']
	},
	{
		id: 'operations-facility',
		name: 'อาคารสถานที่',
		icon: 'School',
		workspaceCode: 'operations',
		order: 10,
		defaultOpen: true,
		paths: ['/staff/facility/buildings']
	},
	{
		id: 'settings-system',
		name: 'ระบบและสิทธิ์',
		icon: 'Settings',
		workspaceCode: 'settings',
		order: 10,
		defaultOpen: true,
		paths: ['/staff/school-settings', '/staff/roles', '/staff/menu', '/staff/features']
	}
];

const definitionsByPath = new Map(
	SIDEBAR_SECTION_DEFINITIONS.flatMap((definition) =>
		definition.paths.map((itemPath) => [itemPath, definition] as const)
	)
);

function workspaceOrder(code: string): number {
	const index = WORKSPACE_ORDER.indexOf(code);
	return index === -1 ? 999 : index;
}

function fallbackSectionForGroup(group: MenuGroup): SidebarSectionDefinition {
	const workspaceCode = group.workspaceCode || 'operations';

	return {
		id: `${workspaceCode}-${group.code}`,
		name: group.name,
		icon: group.icon || 'Circle',
		workspaceCode,
		order: 1000,
		defaultOpen: true,
		paths: []
	};
}

export function buildSidebarNavigation(menuGroups: MenuGroup[]): SidebarWorkspaceSection[] {
	const sectionMap = new Map<string, SidebarMenuSection>();

	for (const group of menuGroups) {
		for (const item of group.items) {
			const definition = definitionsByPath.get(item.path) ?? fallbackSectionForGroup(group);
			const workspaceCode = definition.workspaceCode || group.workspaceCode || 'operations';
			const sectionId = definition.id;
			const section =
				sectionMap.get(sectionId) ??
				({
					id: sectionId,
					name: definition.name,
					icon: definition.icon,
					workspaceCode,
					order: definition.order,
					defaultOpen: definition.defaultOpen ?? false,
					items: []
				} satisfies SidebarMenuSection);

			section.items.push({
				...item,
				groupCode: group.code,
				groupName: group.name,
				workspaceCode
			});
			sectionMap.set(sectionId, section);
		}
	}

	const workspaceMap = new Map<string, SidebarWorkspaceSection>();

	for (const section of sectionMap.values()) {
		const workspace =
			workspaceMap.get(section.workspaceCode) ??
			({
				code: section.workspaceCode,
				name: WORKSPACE_LABELS[section.workspaceCode] ?? section.workspaceCode,
				icon: WORKSPACE_ICONS[section.workspaceCode] ?? 'PanelLeft',
				order: workspaceOrder(section.workspaceCode),
				sections: []
			} satisfies SidebarWorkspaceSection);

		workspace.sections.push(section);
		workspaceMap.set(workspace.code, workspace);
	}

	return Array.from(workspaceMap.values())
		.map((workspace) => ({
			...workspace,
			sections: workspace.sections.sort((a, b) => a.order - b.order || a.name.localeCompare(b.name))
		}))
		.sort((a, b) => a.order - b.order || a.name.localeCompare(b.name));
}
