import { getContext, setContext } from 'svelte';
import type { SupervisionObservation } from '#lib/api/supervision.js';

const context = Symbol('supervision personal return');
const maxAgeMs = 60_000;

function createPersonalReturn() {
	let saved: { scope: string; items: SupervisionObservation[]; readAt: number } | null = null;
	let revision = 0;
	let pendingChanges = 0;

	function clear() {
		saved = null;
		revision += 1;
	}

	return {
		clear,
		read(scope: string) {
			if (!saved || pendingChanges || saved.scope !== scope || Date.now() - saved.readAt > maxAgeMs)
				return null;
			return saved.items;
		},
		captureRead() {
			return revision;
		},
		remember(scope: string, userId: string, items: SupervisionObservation[], readRevision: number) {
			if (pendingChanges || readRevision !== revision) return;
			saved = {
				scope,
				items: items.filter((item) => item.observedUserId === userId),
				readAt: Date.now()
			};
		},
		beginChange() {
			pendingChanges += 1;
			clear();
			return () => {
				pendingChanges -= 1;
				clear();
			};
		}
	};
}

export function providePersonalSupervisionReturn() {
	return setContext(context, createPersonalReturn());
}

export function usePersonalSupervisionReturn() {
	return getContext<ReturnType<typeof createPersonalReturn>>(context);
}
