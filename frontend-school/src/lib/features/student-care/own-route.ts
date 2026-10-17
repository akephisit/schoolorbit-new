import { waitForAuthenticatedUser } from '#lib/auth/settled-user.js';
import { careAssignments, type CareListQuery } from '#lib/api/student-care.js';
import type { ApiRequestOptions } from '#lib/api/client.js';
export async function ownCareAssignments(query: CareListQuery, options: ApiRequestOptions = {}) {
	const user = await waitForAuthenticatedUser();
	if (!user) return [];
	return careAssignments({ ...query, respondentId: user.id }, options);
}
