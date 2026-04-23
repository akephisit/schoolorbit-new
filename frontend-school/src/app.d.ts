// See https://svelte.dev/docs/kit/types#app.d.ts
// for information about these interfaces
declare global {
	namespace App {
		// interface Error {}
		// interface Locals {}
		interface PageData {
			title?: string;
			description?: string;
		}
		// interface PageState {}
		// interface Platform {}
	}

	interface Window {
		// IE-specific — used to exclude IE from iOS user-agent detection
		MSStream?: unknown;
	}

	interface Navigator {
		// Safari iOS non-standard — true when PWA is launched from home screen
		standalone?: boolean;
	}
}

export {};
