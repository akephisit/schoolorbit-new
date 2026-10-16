import { z } from 'zod';
import { validSchoolSubdomain } from '#lib/public-site.js';

const cursor = z.string().refine(validSchoolSubdomain);
const directory = z
	.object({
		schools: z
			.array(z.object({ name: z.string().trim().min(1).max(255), subdomain: cursor }).strict())
			.max(100),
		nextCursor: cursor.nullable()
	})
	.strict();
export type PublicSchools = z.infer<typeof directory>;

export async function readPublicSchools(
	api: string,
	after: string | null,
	fetcher: typeof fetch
): Promise<PublicSchools> {
	if (after !== null && !validSchoolSubdomain(after)) throw new Error('Invalid school cursor');
	const url = new URL('/api/v1/public/schools', api);
	if (after) url.searchParams.set('after', after);
	const response = await fetcher(url, {
		credentials: 'omit',
		cache: 'no-store',
		redirect: 'manual',
		signal: AbortSignal.timeout(8000)
	});
	if (!response.ok) throw new Error('School directory temporarily unavailable');
	const envelope = z
		.object({ success: z.literal(true), data: directory })
		.parse(await response.json());
	if (
		envelope.data.nextCursor &&
		(envelope.data.schools.at(-1)?.subdomain !== envelope.data.nextCursor ||
			envelope.data.nextCursor === after)
	) {
		throw new Error('Invalid school directory pagination');
	}
	return envelope.data;
}
