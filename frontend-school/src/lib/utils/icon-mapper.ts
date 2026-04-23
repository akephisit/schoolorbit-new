// Icon Mapper Utility
// Maps icon name strings to Lucide Svelte components

import * as Icons from 'lucide-svelte';
import { Icon as LucideIcon } from 'lucide-svelte';

type LucideIconCtor = typeof LucideIcon;

/**
 * Map icon name string to Lucide Svelte component
 *
 * Converts kebab-case icon names to PascalCase component names
 * Example: 'layout-dashboard' → LayoutDashboard
 *
 * @param iconName - Icon name in kebab-case (e.g., 'layout-dashboard', 'users', 'shield')
 * @returns Lucide Svelte icon component
 */
export function getIconComponent(iconName?: string): LucideIconCtor {
	if (!iconName) return Icons.Circle;

	// Convert 'layout-dashboard' to 'LayoutDashboard'
	const componentName = iconName
		.split('-')
		.map((part) => part.charAt(0).toUpperCase() + part.slice(1))
		.join('');

	const iconMap = Icons as unknown as Record<string, LucideIconCtor>;
	return iconMap[componentName] ?? Icons.Circle;
}
