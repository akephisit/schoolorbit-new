/** Only safe auth/GET reconciliation is retried. Device POST creation is never replayed. */
export function recoveryDelay(attempt: number, random: number): number | null {
	const delays = [500, 1500, 4000, 10000];
	const delay = delays[attempt];
	return delay === undefined
		? null
		: Math.round(delay * (0.8 + Math.min(1, Math.max(0, random)) * 0.4));
}
