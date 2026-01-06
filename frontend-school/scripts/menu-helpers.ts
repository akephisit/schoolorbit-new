import { glob } from 'glob';
import fs from 'fs';

/**
 * Route metadata structure
 */
export interface RouteMetadata {
	path: string;
	title: string;
	icon?: string;
	group: string;
	order: number;
	permission?: string;
	user_type?: string;
}

/**
 * Scan all route files and extract metadata
 */
export async function scanRoutes(): Promise<RouteMetadata[]> {
	const routes: RouteMetadata[] = [];

	// Find all +page.ts files in (app) routes
	const files = glob.sync('src/routes/(app)/**/+page.ts', {
		ignore: ['**/node_modules/**', '**/.svelte-kit/**']
	});

	for (const file of files) {
		const content = fs.readFileSync(file, 'utf-8');
		const meta = extractMeta(content);

		if (meta?.menu) {
			routes.push({
				path: fileToPath(file),
				title: (meta.menu as any).title,
				icon: (meta.menu as any).icon,
				group: (meta.menu as any).group,
				order: (meta.menu as any).order ?? 999,
				permission: (meta.menu as any).permission,
				user_type: (meta.menu as any).user_type
			});
		}
	}

	return routes;
}

/**
 * Extract _meta.menu from file content
 */
export function extractMeta(content: string): { menu?: unknown } | null {
	try {
		// Match: export const _meta = { ... }
		const metaMatch = content.match(/export\s+const\s+_meta\s*=\s*({[\s\S]*?});/);
		if (!metaMatch) return null;

		const metaCode = metaMatch[1];

		// Match: menu: { ... }
		const menuMatch = metaCode.match(/menu:\s*({[\s\S]*?})\s*(?:,|\})/);
		if (!menuMatch) return null;

		// Convert JavaScript object syntax to valid JSON
		let menuStr = menuMatch[1];

		// Step 0: Remove JavaScript comments (JSON doesn't support comments)
		menuStr = menuStr.replace(/\/\/.*$/gm, ''); // Remove single-line comments
		menuStr = menuStr.replace(/\/\*[\s\S]*?\*\//g, ''); // Remove multi-line comments

		// Step 1: Replace single quotes with double quotes for string values
		// But be careful with quotes inside strings
		menuStr = menuStr.replace(/'([^']*)'/g, '"$1"');

		// Step 2: Wrap unquoted keys in double quotes
		menuStr = menuStr.replace(/(\w+):/g, '"$1":');

		// Step 3: Remove trailing commas before closing braces/brackets
		menuStr = menuStr.replace(/,(\s*[}\]])/g, '$1');

		// Try to parse as JSON
		try {
			const menuObj = JSON.parse(menuStr);
			return { menu: menuObj };
		} catch (parseError) {
			// If JSON.parse fails, log detailed error for debugging
			console.error(`Failed to parse menu metadata as JSON:`, {
				error: parseError,
				converted: menuStr,
				original: menuMatch[1]
			});
			return null;
		}
	} catch (error) {
		console.error(`Failed to extract meta from content:`, error);
		return null;
	}
}

/**
 * Convert file path to URL path
 * Example: src/routes/(app)/admin/+page.ts → /admin
 */
export function fileToPath(filePath: string): string {
	return (
		filePath
			.replace('src/routes/(app)', '')
			.replace('/+page.ts', '')
			.replace(/\/\(.*?\)/g, '') || // Remove route groups
		'/'
	);
}

/**
 * Generate code (slug) from title
 * Example: "ระบบจัดการ" → "ระบบจัดการ" (keep as is for DB)
 */
export function slugify(text: string): string {
	return text
		.toLowerCase()
		.replace(/[^\w\s-]/g, '')
		.replace(/\s+/g, '_');
}
