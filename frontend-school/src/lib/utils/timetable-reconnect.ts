export function reconnectDelayMs(attempt: number, random: () => number = Math.random): number {
	const base = Math.min(30_000, 1_000 * 2 ** attempt);
	return Math.round(base * (0.8 + random() * 0.4));
}
