import type { PageLoad } from './$types';

export const load: PageLoad = async ({ params }) => {
    return { title: 'รายละเอียดใบสมัคร', periodId: params.id, appId: params.appId };
};
