import { scanRoutes } from './menu-helpers.ts';

interface RouteRegistrationResponse {
	success: boolean;
	registered: number;
	message: string;
}

function requiredEnvironment(name: 'PUBLIC_BACKEND_URL' | 'DEPLOY_KEY' | 'SUBDOMAIN'): string {
	const value = process.env[name]?.trim();
	if (!value) {
		throw new Error(`${name} is required for menu synchronization`);
	}
	return value;
}

async function synchronizeMenuRoutes(): Promise<void> {
	const backendUrl = requiredEnvironment('PUBLIC_BACKEND_URL').replace(/\/+$/, '');
	const deployKey = requiredEnvironment('DEPLOY_KEY');
	const subdomain = requiredEnvironment('SUBDOMAIN');
	const routes = await scanRoutes(process.argv[2] ?? process.cwd());

	if (routes.length === 0) {
		throw new Error('No menu routes found; refusing to synchronize an empty desired state');
	}

	const seenPaths = new Set<string>();
	for (const route of routes) {
		if (seenPaths.has(route.path)) {
			throw new Error(`Duplicate menu route path: ${route.path}`);
		}
		seenPaths.add(route.path);
	}

	const response = await fetch(`${backendUrl}/api/admin/routes/sync`, {
		method: 'POST',
		headers: {
			'Content-Type': 'application/json',
			'X-Deploy-Key': deployKey,
			'X-School-Subdomain': subdomain
		},
		body: JSON.stringify({
			routes,
			environment: process.env.NODE_ENV ?? 'production'
		})
	});

	if (!response.ok) {
		throw new Error(`Backend returned ${response.status} during menu synchronization`);
	}

	const result: RouteRegistrationResponse = await response.json();
	if (result.success !== true) {
		throw new Error('Backend did not confirm menu synchronization');
	}

	console.log(result.message || `Synchronized ${result.registered} menu routes`);
}

synchronizeMenuRoutes().catch((error) => {
	console.error(error instanceof Error ? error.message : 'Menu synchronization failed');
	process.exitCode = 1;
});
