// Menu configuration with permissions
import type { ComponentType } from 'svelte';
import {
	LayoutDashboard,
	Users,
	GraduationCap,
	BookOpen,
	School,
	Calendar,
	Shield,
	Settings
} from 'lucide-svelte';

export interface MenuItem {
	name: string;
	icon: ComponentType;
	href: string;
	permission: string; // Required permission to see this menu
	group?: 'main' | 'admin' | 'settings';
}

export const menuItems: MenuItem[] = [
	// Main Navigation
	{
		name: 'แดชบอร์ด',
		icon: LayoutDashboard,
		href: '/dashboard',
		permission: 'dashboard.view',
		group: 'main'
	},
	{
		name: 'นักเรียน',
		icon: Users,
		href: '/students',
		permission: 'students.view',
		group: 'main'
	},
	{
		name: 'บุคลากร',
		icon: GraduationCap,
		href: '/staff',
		permission: 'staff.manage', // Changed from users.view - only admin/dept_head can manage staff
		group: 'main'
	},
	{
		name: 'รายวิชา',
		icon: BookOpen,
		href: '/subjects',
		permission: 'subjects.view',
		group: 'main'
	},
	{
		name: 'ห้องเรียน',
		icon: School,
		href: '/classes',
		permission: 'classes.view',
		group: 'main'
	},
	{
		name: 'ปฏิทิน',
		icon: Calendar,
		href: '/calendar',
		permission: 'calendar.view',
		group: 'main'
	},

	// Admin Section
	{
		name: 'จัดการบทบาท',
		icon: Shield,
		href: '/admin/roles',
		permission: 'roles.view',
		group: 'admin'
	},

	// Settings
	{
		name: 'ตั้งค่า',
		icon: Settings,
		href: '/settings',
		permission: 'settings.view',
		group: 'settings'
	}
];

// Helper function to filter menus by permission
export function filterMenusByPermission(items: MenuItem[], userPermissions: string[]): MenuItem[] {
	// Admin wildcard - show all menus
	if (userPermissions.includes('*')) {
		return items;
	}

	// Filter by permission
	return items.filter((item) => userPermissions.includes(item.permission));
}

// Get menus by group
export function getMenusByGroup(items: MenuItem[], group: MenuItem['group']): MenuItem[] {
	return items.filter((item) => item.group === group);
}
