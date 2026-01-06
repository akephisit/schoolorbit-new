/**
 * Student Profile (Student self-service)
 */

export const _meta = {
    menu: {
        title: 'ข้อมูลส่วนตัว',
        icon: 'User',
        group: 'main',
        order: 10,
        user_type: 'student',
        permission: 'student.read.own'
    }
};

export const load = async () => {
    return {
        title: 'ข้อมูลส่วนตัว'
    };
};
