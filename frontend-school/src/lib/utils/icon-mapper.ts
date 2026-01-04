// Icon Mapper Utility
// Maps icon name strings to Lucide Svelte components

import * as Icons from 'lucide-svelte';
import type { ComponentType } from 'svelte';

/**
 * Map icon name string to Lucide Svelte component
 *
 * Converts kebab-case icon names to PascalCase component names
 * Example: 'layout-dashboard' → LayoutDashboard
 *
 * @param iconName - Icon name in kebab-case (e.g., 'layout-dashboard', 'users', 'shield')
 * @returns Lucide Svelte icon component
 */
export function getIconComponent(iconName?: string): ComponentType {
	if (!iconName) {
		return Icons.Circle;
	}

	// Convert 'layout-dashboard' to 'LayoutDashboard'
	const componentName = iconName
		.split('-')
		.map((part) => part.charAt(0).toUpperCase() + part.slice(1))
		.join('');

	// Get component from Icons (use any to bypass type checking)
	const IconComponent = (Icons as any)[componentName];

	// Fallback to Circle if not found
	return (IconComponent as ComponentType) || Icons.Circle;
}
