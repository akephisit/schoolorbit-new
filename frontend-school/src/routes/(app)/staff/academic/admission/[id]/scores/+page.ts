import type { PageLoad } from './$types';
export const load: PageLoad = async ({ params }) => ({
    title: 'จัดการคะแนนสอบ',
    periodId: params.id
});
