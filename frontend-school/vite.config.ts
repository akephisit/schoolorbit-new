import adapter from '@sveltejs/adapter-cloudflare';
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';
import tailwindcss from '@tailwindcss/vite';
import { sveltekit } from '@sveltejs/kit/vite';
import { fileURLToPath, URL } from 'node:url';
import { defineConfig, type Plugin } from 'vite';

const wordExporterModule = fileURLToPath(
	new URL('./src/lib/question-bank/word-export', import.meta.url)
);
const wordExporterServerStub = fileURLToPath(
	new URL('./src/lib/question-bank/word-export.server.ts', import.meta.url)
);
const certificateRendererModule = fileURLToPath(
	new URL('./src/lib/certificates/renderer', import.meta.url)
);
const certificateRendererServerStub = fileURLToPath(
	new URL('./src/lib/certificates/renderer.server.ts', import.meta.url)
);
const browserOnlyHeavyDependencyServerStub = fileURLToPath(
	new URL('./src/lib/utils/browser-only-heavy-dependency.server.ts', import.meta.url)
);
const browserOnlyHeavyDependencies = new Set([
	'exceljs',
	'heic2any',
	'pdf-lib',
	'pdfjs-dist',
	'pdfmake/build/pdfmake',
	'qrcode',
	'ssf'
]);

function clientOnlyWordExporterPlugin(): Plugin {
	return {
		name: 'client-only-word-exporter',
		enforce: 'pre',
		resolveId(source) {
			if (
				this.environment.name === 'ssr' &&
				(source === '#lib/question-bank/word-export.js' || source === wordExporterModule)
			) {
				return wordExporterServerStub;
			}
		}
	};
}

function clientOnlyCertificateRendererPlugin(): Plugin {
	return {
		name: 'client-only-certificate-renderer',
		enforce: 'pre',
		resolveId(source) {
			if (
				this.environment.name === 'ssr' &&
				(source === '#lib/certificates/renderer.js' || source === certificateRendererModule)
			) {
				return certificateRendererServerStub;
			}
		}
	};
}

function clientOnlyHeavyDependenciesPlugin(): Plugin {
	return {
		name: 'client-only-heavy-dependencies',
		enforce: 'pre',
		resolveId(source) {
			if (this.environment.name === 'ssr' && browserOnlyHeavyDependencies.has(source)) {
				return browserOnlyHeavyDependencyServerStub;
			}
		}
	};
}

export default defineConfig({
	plugins: [
		clientOnlyWordExporterPlugin(),
		clientOnlyCertificateRendererPlugin(),
		clientOnlyHeavyDependenciesPlugin(),
		tailwindcss(),
		sveltekit({
			// Consult https://svelte.dev/docs/kit/integrations
			// for more information about preprocessors
			preprocess: vitePreprocess(),

			// adapter-cloudflare for Cloudflare Workers deployment
			adapter: adapter(),
			version: { pollInterval: 0 }
		})
	],
	build: {
		target: 'esnext',
		sourcemap: false,
		reportCompressedSize: false,
		chunkSizeWarningLimit: 1500
	}
});
