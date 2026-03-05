import type { PageLoad } from './$types';
export const load: PageLoad = async ({ params }) => ({
    title: 'รายงานตัวนักเรียน',
    periodId: params.id
});
