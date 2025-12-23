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
import { hasPermission } from '$lib/utils/permissions';

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
        permission: 'dashboard.read',
        group: 'main'
    },
    {
        name: 'นักเรียน',
        icon: Users,
        href: '/students',
        permission: 'students.read',
        group: 'main'
    },
    {
        name: 'บุคลากร',
        icon: GraduationCap,
        href: '/staff',
        permission: 'staff.read',
        group: 'main'
    },
    {
        name: 'รายวิชา',
        icon: BookOpen,
        href: '/subjects',
        permission: 'subjects.read',
        group: 'main'
    },
    {
        name: 'ห้องเรียน',
        icon: School,
        href: '/classes',
        permission: 'classes.read',
        group: 'main'
    },
    {
        name: 'ปฏิทิน',
        icon: Calendar,
        href: '/calendar',
        permission: 'calendar.read',
        group: 'main'
    },

    // Admin Section
    {
        name: 'จัดการบทบาท',
        icon: Shield,
        href: '/admin/roles',
        permission: 'roles.read',
        group: 'admin'
    },

    // Settings
    {
        name: 'ตั้งค่า',
        icon: Settings,
        href: '/settings',
        permission: 'settings.read',
        group: 'settings'
    }
];

/**
 * Filter menus by user permissions
 * Uses granular permission checking with wildcard support
 */
export function filterMenusByPermission(items: MenuItem[], userPermissions: string[]): MenuItem[] {
    // Admin wildcard - show all menus
    if (userPermissions.includes('*')) {
        return items;
    }

    // Filter by permission using granular permission checking
    return items.filter((item) => hasPermission(userPermissions, item.permission));
}

// Get menus by group
export function getMenusByGroup(items: MenuItem[], group: MenuItem['group']): MenuItem[] {
    return items.filter((item) => item.group === group);
}
