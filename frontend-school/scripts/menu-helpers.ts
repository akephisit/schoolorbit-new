import { glob } from 'glob';
import fs from 'fs';
import path from 'path';

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
                title: meta.menu.title,
                icon: meta.menu.icon,
                group: meta.menu.group,
                order: meta.menu.order ?? 999,
                permission: meta.menu.permission
            });
        }
    }

    return routes;
}

/**
 * Extract _meta.menu from file content
 */
export function extractMeta(content: string): { menu?: any } | null {
    try {
        // Match: export const _meta = { ... }
        const metaMatch = content.match(/export\s+const\s+_meta\s*=\s*({[\s\S]*?});/);
        if (!metaMatch) return null;

        const metaCode = metaMatch[1];

        // Match: menu: { ... }
        const menuMatch = metaCode.match(/menu:\s*({[\s\S]*?})\s*(?:,|\})/);
        if (!menuMatch) return null;

        // Convert JavaScript object syntax to valid JSON
        // Replace single quotes with double quotes and remove trailing commas
        const menuStr = menuMatch[1]
            .replace(/'/g, '"')  // Single quotes to double quotes
            .replace(/(\w+):/g, '"$1":')  // Wrap keys in double quotes
            .replace(/,(\s*[}\]])/g, '$1');  // Remove trailing commas

        // Parse as JSON instead of eval
        const menuObj = JSON.parse(menuStr);

        return { menu: menuObj };
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
    return filePath
        .replace('src/routes/(app)', '')
        .replace('/+page.ts', '')
        .replace(/\/\(.*?\)/g, '') // Remove route groups
        || '/';
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
