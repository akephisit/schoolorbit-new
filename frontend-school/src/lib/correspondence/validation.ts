import { tick } from 'svelte';

export type CorrespondenceFieldErrors = Record<string, string | undefined>;

/** Call after selecting the form step; reveal folded fields before focusing them. */
export async function focusCorrespondenceField(id: string) {
	await tick();
	const target = document.getElementById(id);
	if (!target) return;
	for (let parent = target.parentElement; parent; parent = parent.parentElement) {
		if (parent instanceof HTMLDetailsElement) parent.open = true;
	}
	await tick();
	target.focus({ preventScroll: true });
	target.scrollIntoView({ block: 'center' });
}
