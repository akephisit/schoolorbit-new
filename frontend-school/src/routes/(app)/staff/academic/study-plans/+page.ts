/**
 * Study Plans Management Page
 * จัดการหลักสูตรสถานศึกษา (ฉบับปรับปรุง พ.ศ. ...)
 */

export const _meta = {
    menu: {
        title: 'หลักสูตรสถานศึกษา',
        icon: 'GraduationCap',
        group: 'academic',
        order: 3,
        user_type: 'staff',
        permission: 'academic_curriculum.read.all'
    }
};

export const load = async () => {
    return {
        title: _meta.menu.title
    };
};
