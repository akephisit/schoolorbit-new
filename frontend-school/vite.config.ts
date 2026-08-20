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
	'xlsx'
]);

function clientOnlyWordExporterPlugin(): Plugin {
	return {
		name: 'client-only-word-exporter',
		enforce: 'pre',
		resolveId(source) {
			if (this.environment.name === 'ssr' && source === wordExporterModule) {
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
			if (this.environment.name === 'ssr' && source === certificateRendererModule) {
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
		sveltekit()
	],
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
