type BudgetItem = { id: string; maxScore: string; lifecycle: string };

/** Canonical scores have at most two decimal places; sum in hundredths. */
export function scoreItemBudget(phaseMaximum: string, items: BudgetItem[], editingId?: string) {
	const active = items.filter((item) => item.lifecycle === 'active');
	const allocated = active.reduce((sum, item) => sum + Math.round(Number(item.maxScore) * 100), 0);
	const remaining = Math.round(Number(phaseMaximum) * 100) - allocated;
	const prior = Math.round(
		Number(active.find((item) => item.id === editingId)?.maxScore ?? 0) * 100
	);
	return {
		allocated: allocated / 100,
		remaining: remaining / 100,
		maximumForItem: (prior + Math.max(0, remaining)) / 100,
		canAdd: remaining > 0
	};
}
