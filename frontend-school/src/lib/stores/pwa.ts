import { writable } from 'svelte/store';

// BeforeInstallPrompt event type
export interface BeforeInstallPromptEvent extends Event {
	prompt: () => Promise<void>;
	userChoice: Promise<{ outcome: 'accepted' | 'dismissed' }>;
}

// PWA install state
interface PWAState {
	deferredPrompt: BeforeInstallPromptEvent | null;
	isInstalled: boolean;
}

// Create writable store
function createPWAStore() {
	const { subscribe, set, update } = writable<PWAState>({
		deferredPrompt: null,
		isInstalled: false
	});

	return {
		subscribe,
		setPrompt: (event: BeforeInstallPromptEvent | null) => {
			update((state) => ({ ...state, deferredPrompt: event }));
		},
		setInstalled: (installed: boolean) => {
			update((state) => ({ ...state, isInstalled: installed, deferredPrompt: null }));
		},
		reset: () => {
			set({ deferredPrompt: null, isInstalled: false });
		}
	};
}

export const pwaStore = createPWAStore();

// Initialize PWA listeners (call once in app root)
export function initPWA() {
	// Check if already installed
	if (typeof window !== 'undefined' && window.matchMedia('(display-mode: standalone)').matches) {
		pwaStore.setInstalled(true);
	}

	// Listen for install prompt
	const handleBeforeInstallPrompt = (e: Event) => {
		e.preventDefault();
		pwaStore.setPrompt(e as BeforeInstallPromptEvent);
		console.log('💾 PWA install prompt available');
	};

	// Listen for successful installation
	const handleAppInstalled = () => {
		console.log('✅ PWA installed successfully!');
		pwaStore.setInstalled(true);
	};

	if (typeof window !== 'undefined') {
		window.addEventListener('beforeinstallprompt', handleBeforeInstallPrompt);
		window.addEventListener('appinstalled', handleAppInstalled);
	}
}
