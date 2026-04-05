import tailwindcss from '@tailwindcss/vite';
import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vite';
import { scanRoutes } from './scripts/menu-helpers';

/**
 * Auto-register menu items during build
 */
function menuRegistryPlugin() {
	return {
		name: 'menu-registry',
		writeBundle: async () => {
			// Only run in production builds when VITE_DEPLOY_KEY is set
			const deployKey = process.env.VITE_DEPLOY_KEY;
			if (!deployKey) {
				console.log('⏭️  Skipping menu registration (no VITE_DEPLOY_KEY)');
				return;
			}

			// Require SUBDOMAIN for registration
			const subdomain = process.env.SUBDOMAIN;
			if (!subdomain) {
				console.log('⏭️  Skipping menu registration (no SUBDOMAIN)');
				console.log('   This is expected for non-deployment builds');
				return;
			}

			try {
				console.log('📋 Scanning routes for menu metadata...');

				const routes = await scanRoutes();

				if (routes.length === 0) {
					console.log('⚠️  No routes with menu metadata found');
					return;
				}

				console.log(`✅ Found ${routes.length} menu items`);
				console.log(`📍 Registering for school: ${subdomain}`);
				console.log('📤 Sending to backend...');

				// Get backend URL from env
				const backendUrl = process.env.PUBLIC_BACKEND_URL || 'https://school-api.schoolorbit.app';

				// POST to backend
				const response = await fetch(`${backendUrl}/api/admin/routes/register`, {
					method: 'POST',
					headers: {
						'Content-Type': 'application/json',
						'X-Deploy-Key': deployKey,
						'X-School-Subdomain': subdomain
					},
					body: JSON.stringify({
						routes,
						timestamp: new Date().toISOString(),
						environment: process.env.NODE_ENV || 'production'
					})
				});

				if (!response.ok) {
					const error = await response.text();
					throw new Error(`Backend returned ${response.status}: ${error}`);
				}

				const result = await response.json();
				console.log(`✅ ${result.message}`);
				console.log('🎉 Menu registration complete!');
			} catch (error) {
				console.error('❌ Failed to register menu items:', error);
				console.error('⚠️  Build will continue, but menu items were not registered');
				// Don't fail the build, just warn
			}
		}
	};
}

export default defineConfig({
	plugins: [tailwindcss(), sveltekit(), menuRegistryPlugin()],
	optimizeDeps: {
		include: ['html2pdf.js']
	},
	ssr: {
		external: ['html2pdf.js']
	},
	build: {
		target: 'esnext',
		sourcemap: false,
		reportCompressedSize: false,
		chunkSizeWarningLimit: 1500
	}
});
