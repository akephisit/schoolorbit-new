import type { PageLoad } from './$types';
export const load: PageLoad = async ({ params }) => ({
    title: 'เพิ่มใบสมัครนักเรียน',
    periodId: params.id
});
