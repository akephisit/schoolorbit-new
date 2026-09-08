type VisibilityDependencies<Timer> = {
	isHidden(): boolean;
	setTimer(callback: () => void, delay: number): Timer;
	clearTimer(timer: Timer): void;
	addListener(listener: () => void): void;
	removeListener(listener: () => void): void;
	onPause(): void;
	onResume(): void;
};

/** A transport owner remains subscribed while paused, until explicitly stopped. */
export function createVisibilityIdle<Timer>(dependencies: VisibilityDependencies<Timer>) {
	let active = false;
	let paused = false;
	let timer: Timer | null = null;
	let generation = 0;
	function clearTimer() {
		generation++;
		if (timer !== null) dependencies.clearTimer(timer);
		timer = null;
	}
	function changed() {
		if (!active) return;
		if (dependencies.isHidden()) {
			if (timer !== null || paused) return;
			const current = generation;
			timer = dependencies.setTimer(() => {
				if (!active || current !== generation) return;
				timer = null;
				if (!dependencies.isHidden()) return;
				paused = true;
				dependencies.onPause();
			}, 60_000);
		} else {
			clearTimer();
			if (!paused) return;
			paused = false;
			dependencies.onResume();
		}
	}
	return {
		get paused() {
			return paused;
		},
		start() {
			if (active) return;
			active = true;
			dependencies.addListener(changed);
			changed();
		},
		stop() {
			active = false;
			paused = false;
			clearTimer();
			dependencies.removeListener(changed);
		}
	};
}

export function browserVisibilityDependencies() {
	return {
		isHidden: () => typeof document !== 'undefined' && document.hidden,
		setTimer: (callback: () => void, delay: number) => setTimeout(callback, delay),
		clearTimer: (timer: ReturnType<typeof setTimeout>) => clearTimeout(timer),
		addListener: (listener: () => void) =>
			typeof document !== 'undefined' && document.addEventListener('visibilitychange', listener),
		removeListener: (listener: () => void) =>
			typeof document !== 'undefined' && document.removeEventListener('visibilitychange', listener)
	};
}
