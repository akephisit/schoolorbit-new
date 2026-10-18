import type { CareListQuery } from '#lib/api/student-care.js';
export function careListQuery(url: URL): CareListQuery {
	return {
		afterId: url.searchParams.get('afterId') ?? undefined,
		roundId: url.searchParams.get('roundId') ?? undefined,
		studentId: url.searchParams.get('studentId') ?? undefined,
		status: url.searchParams.get('status') ?? undefined,
		followUpBefore: url.searchParams.get('followUpBefore') ?? undefined,
		limit: 50
	};
}
