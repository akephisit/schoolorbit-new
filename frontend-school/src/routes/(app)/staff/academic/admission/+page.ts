/**
 * Admission Management — รายการรอบรับสมัครทั้งหมด
 */

export const _meta = {
    menu: {
        title: 'รับสมัครนักเรียน',
        icon: 'ClipboardList',
        group: 'academic',
        order: 40,
        user_type: 'staff',
        permission: 'admission.read.all'
    }
};

export const load = async () => {
    return {
        title: _meta.menu.title
    };
};
