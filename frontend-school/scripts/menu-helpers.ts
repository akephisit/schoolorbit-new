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
 * Extract meta.menu from file content
 */
export function extractMeta(content: string): { menu?: any } | null {
    try {
        // Match: export const meta = { ... }
        const metaMatch = content.match(/export\s+const\s+meta\s*=\s*({[\s\S]*?});/);
        if (!metaMatch) return null;

        const metaCode = metaMatch[1];

        // Match: menu: { ... }
        const menuMatch = metaCode.match(/menu:\s*({[\s\S]*?})\s*(?:,|\})/);
        if (!menuMatch) return null;

        // Evaluate the menu object
        // SAFETY: This is build-time code scanning our own files
        const menuObj = eval(`(${menuMatch[1]})`);

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
