import { getPublicSchoolAppearance } from './api.js';
import { AppearanceController } from './controller.js';
import {
	FONTS,
	schoolAppearanceCss,
	validateAppearanceResponse,
	type PreviewFont,
	type SchoolAppearanceResponse
} from './model.js';

export async function prepareSchoolFont(font: PreviewFont): Promise<void> {
	const family = FONTS.find((option) => option.value === font)?.family;
	if (!family || !document.fonts) throw new Error('โหลดฟอนต์ไม่สำเร็จ');
	let timer: ReturnType<typeof setTimeout> | undefined;
	try {
		await Promise.race([
			Promise.all(
				[400, 500, 600, 700].map(async (weight) => {
					const faces = await document.fonts.load(
						`${weight} 16px '${family}'`,
						'กขคง SchoolOrbit 0123456789'
					);
					if (!faces.length) throw new Error('โหลดฟอนต์ไม่สำเร็จ');
				})
			),
			new Promise<never>((_, reject) => {
				timer = setTimeout(() => reject(new Error('โหลดฟอนต์ไม่สำเร็จ')), 3000);
			})
		]);
	} finally {
		clearTimeout(timer);
	}
}

function readAppliedAppearance(): SchoolAppearanceResponse {
	const css = getComputedStyle(document.documentElement);
	const property = (name: string) => css.getPropertyValue(`--school-appearance-${name}`).trim();
	const result = validateAppearanceResponse({
		revision: Number(property('revision')),
		appearance: {
			palette: property('palette'),
			font: property('font'),
			density: property('density'),
			primary: property('primary') === 'none' ? null : property('primary')
		}
	});
	if (!result) throw new Error('ไม่พบรูปแบบโรงเรียนที่โหลดไว้');
	return result;
}
function install(value: SchoolAppearanceResponse) {
	let style = document.getElementById('school-appearance-runtime');
	if (!style) {
		style = document.createElement('style');
		style.id = 'school-appearance-runtime';
		document.head.append(style);
	}
	style.textContent = schoolAppearanceCss(value);
	document.documentElement.style.removeProperty('--school-font-family');
	delete document.documentElement.dataset.schoolFontFallback;
	const color = getComputedStyle(document.documentElement).getPropertyValue('--background').trim();
	document.querySelector('meta[name="theme-color"]')?.setAttribute('content', color);
}
let active: AppearanceController | undefined;
let channel: BroadcastChannel | undefined;

export async function applySavedSchoolAppearance(value: SchoolAppearanceResponse) {
	if (!active) return 'font-failed' as const;
	const result = await active.apply(value, true);
	channel?.postMessage('appearance-saved');
	return result;
}

/** Root layout owns one listener set for the document, independent of SPA routes. */
export function startSchoolAppearance(): () => void {
	const controller = new AppearanceController(readAppliedAppearance(), prepareSchoolFont, install);
	active = controller;
	let checkedAt = Date.now();
	let request: AbortController | undefined;
	let disposed = false;
	const refresh = async (force = false) => {
		if (disposed || request || (!force && Date.now() - checkedAt < 30_000)) return;
		checkedAt = Date.now();
		request = new AbortController();
		const pending = request;
		const timer = setTimeout(() => pending.abort(), 3000);
		try {
			const value = await getPublicSchoolAppearance({ signal: pending.signal });
			if (!disposed && !pending.signal.aborted) await controller.apply(value);
		} catch {
			/* Preserve the applied document theme on transient public-read failure. */
		} finally {
			clearTimeout(timer);
			if (request === pending) request = undefined;
		}
	};
	const visibility = () => {
		if (document.visibilityState === 'visible') void refresh();
	};
	document.addEventListener('visibilitychange', visibility);
	if ('BroadcastChannel' in window) {
		channel = new BroadcastChannel('schoolorbit:school-appearance');
		channel.onmessage = () => {
			void refresh(true);
		};
	}
	return () => {
		disposed = true;
		request?.abort();
		controller.dispose();
		document.removeEventListener('visibilitychange', visibility);
		channel?.close();
		channel = undefined;
		if (active === controller) active = undefined;
	};
}
