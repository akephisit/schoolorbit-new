import type { SchoolAppearanceResponse, PreviewFont } from './model.js';

/** Coordinates font preparation and atomic CSS installation; obsolete work cannot repaint. */
export class AppearanceController {
	private generation = 0;
	private stopped = false;
	private appliedRevision: number;
	private preparing = false;
	private observed: SchoolAppearanceResponse;
	private prepare: (font: PreviewFont) => Promise<void>;
	private install: (value: SchoolAppearanceResponse) => void;
	constructor(
		initial: SchoolAppearanceResponse,
		prepare: (font: PreviewFont) => Promise<void>,
		install: (value: SchoolAppearanceResponse) => void
	) {
		this.observed = initial;
		this.appliedRevision = initial.revision;
		this.prepare = prepare;
		this.install = install;
	}
	get current() {
		return this.observed;
	}
	async apply(
		value: SchoolAppearanceResponse,
		retry = false
	): Promise<'applied' | 'stale' | 'font-failed'> {
		if (
			this.stopped ||
			value.revision < this.observed.revision ||
			(!retry &&
				value.revision === this.observed.revision &&
				(this.appliedRevision === value.revision || this.preparing))
		)
			return 'stale';
		this.observed = value;
		const generation = ++this.generation;
		this.preparing = true;
		try {
			await this.prepare(value.appearance.font);
		} catch {
			if (generation !== this.generation || this.stopped) return 'stale';
			this.preparing = false;
			return 'font-failed';
		}
		if (this.stopped || generation !== this.generation) return 'stale';
		this.install(value);
		this.appliedRevision = value.revision;
		this.preparing = false;
		return 'applied';
	}
	dispose() {
		this.stopped = true;
		this.generation++;
	}
}
