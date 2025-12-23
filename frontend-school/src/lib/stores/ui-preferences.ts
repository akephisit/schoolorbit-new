import { writable } from 'svelte/store';
import { browser } from '$app/environment';

// UI Preferences type
interface UIPreferences {
    sidebarCollapsed: boolean;
    theme: 'light' | 'dark' | 'system';
}

const defaultPreferences: UIPreferences = {
    sidebarCollapsed: false,
    theme: 'system'
};

// Helper to get preferences from localStorage
function getStoredPreferences(): UIPreferences {
    if (!browser) return defaultPreferences;

    try {
        const stored = localStorage.getItem('ui-preferences');
        if (stored) {
            return { ...defaultPreferences, ...JSON.parse(stored) };
        }
    } catch (error) {
        console.error('Failed to load UI preferences:', error);
    }

    return defaultPreferences;
}

// Create the store
function createUIPreferencesStore() {
    const { subscribe, set, update } = writable<UIPreferences>(getStoredPreferences());

    return {
        subscribe,
        setSidebarCollapsed: (collapsed: boolean) => {
            update(prefs => {
                const updated = { ...prefs, sidebarCollapsed: collapsed };
                if (browser) {
                    localStorage.setItem('ui-preferences', JSON.stringify(updated));
                }
                return updated;
            });
        },
        setTheme: (theme: 'light' | 'dark' | 'system') => {
            update(prefs => {
                const updated = { ...prefs, theme };
                if (browser) {
                    localStorage.setItem('ui-preferences', JSON.stringify(updated));
                }
                return updated;
            });
        },
        reset: () => {
            set(defaultPreferences);
            if (browser) {
                localStorage.removeItem('ui-preferences');
            }
        }
    };
}

export const uiPreferences = createUIPreferencesStore();
