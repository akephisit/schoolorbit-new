use crate::{
    models::{LeaveCountingMode, LeaveKind, LeavePolicyUnit},
    policy::require_configuration_manage,
};
use school_authorization::ActorContext;
use school_errors::AppError;
use school_staff::career::StaffPersonnelType;
use serde::{Deserialize, Serialize};

const CIVIL_SERVICE_LEAVE_URL: &str =
    "https://hr.onab.go.th/th/file/get/file/20230328d3412f9e1039789b70d8faf2e1397d70114216.pdf#page=3";
const SCHOOL_EXIT_EXAMPLE_URL: &str =
    "https://webschool.chaiyaphum3.go.th/oit/oit68/202508151762328088.pdf#page=36";
const FULL_DAY_WORKFLOW_GUIDANCE: &str =
    "ระบบรับเฉพาะคำขอเต็มวันภายในปีสิทธิ์เดียว ใช้ช่องทางที่หน่วยงานกำหนดสำหรับการลาครึ่งวันหรือการลาต่อเนื่องข้ามปีงบประมาณ";
const DOMESTIC_TRAVEL_ORDER_URL: &str = concat!(
    "https://audit.secondarysurin.go.th/wp-content/uploads/2025/12/",
    "%E0%B8%AB%E0%B8%99%E0%B8%B1%E0%B8%87%E0%B8%AA%E0%B8%B7%E0%B8%AD",
    "%E0%B8%84%E0%B8%B3%E0%B8%AA%E0%B8%B1%E0%B9%88%E0%B8%87",
    "%E0%B8%A1%E0%B8%AD%E0%B8%9A%E0%B8%AD%E0%B8%B3%E0%B8%99%E0%B8%B2%E0%B8%88.pdf"
);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum LeavePolicyPresetId {
    SickLeave,
    PersonalLeave,
    SchoolExit,
    OfficialTravel,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct LeavePolicyPreset {
    pub id: LeavePolicyPresetId,
    pub name: String,
    pub kind: LeaveKind,
    /// Reference to copy into a draft. For local school exit the school supplies its own reference.
    pub source_reference: Option<String>,
    pub source_url: String,
    pub personnel_types: Vec<StaffPersonnelType>,
    pub unit: LeavePolicyUnit,
    pub counting_mode: LeaveCountingMode,
    pub quota_year_start_month: Option<u32>,
    pub guidance: Vec<String>,
}

pub fn list_policy_presets(actor: &ActorContext) -> Result<Vec<LeavePolicyPreset>, AppError> {
    require_configuration_manage(actor)?;

    Ok(vec![
        LeavePolicyPreset {
            id: LeavePolicyPresetId::SickLeave,
            name: "ลาป่วย (ข้าราชการ)".into(),
            kind: LeaveKind::Leave,
            source_reference: Some(
                "ระเบียบสำนักนายกรัฐมนตรีว่าด้วยการลาของข้าราชการ พ.ศ. 2555 ข้อ 10, 12 และ 18"
                    .into(),
            ),
            source_url: CIVIL_SERVICE_LEAVE_URL.into(),
            personnel_types: vec![StaffPersonnelType::CivilServant],
            unit: LeavePolicyUnit::Day,
            counting_mode: LeaveCountingMode::WorkingDays,
            quota_year_start_month: Some(10),
            guidance: vec![
                "สถิติการลาป่วยทั่วไปนับเฉพาะวันทำการ โดยเริ่มปีงบประมาณวันที่ 1 ตุลาคม"
                    .into(),
                "ตามข้อ 18 ให้ยื่นก่อนหรือในวันลา หากจำเป็นให้ยื่นในวันแรกที่กลับมาปฏิบัติงาน"
                    .into(),
                "ลาป่วยต่อเนื่องตั้งแต่ 30 วันให้ตรวจใบรับรองแพทย์ สำหรับระยะสั้นกว่านี้ ผู้อนุญาตอาจขอเพิ่มตามกรณี ให้นับระยะต่อเนื่องแยกจากยอดวันทำการ"
                    .into(),
                "ตรวจผู้มีอำนาจอนุญาตตามระยะลาต่อเนื่อง และตรวจสิทธิรับเงินเดือนแยกจากการอนุญาตลา"
                    .into(),
                FULL_DAY_WORKFLOW_GUIDANCE.into(),
            ],
        },
        LeavePolicyPreset {
            id: LeavePolicyPresetId::PersonalLeave,
            name: "ลากิจส่วนตัว (ข้าราชการ)".into(),
            kind: LeaveKind::Leave,
            source_reference: Some(
                "ระเบียบสำนักนายกรัฐมนตรีว่าด้วยการลาของข้าราชการ พ.ศ. 2555 ข้อ 10, 12 และ 21"
                    .into(),
            ),
            source_url: CIVIL_SERVICE_LEAVE_URL.into(),
            personnel_types: vec![StaffPersonnelType::CivilServant],
            unit: LeavePolicyUnit::Day,
            counting_mode: LeaveCountingMode::WorkingDays,
            quota_year_start_month: Some(10),
            guidance: vec![
                "สถิติลากิจส่วนตัวทั่วไปนับเฉพาะวันทำการ โดยเริ่มปีงบประมาณวันที่ 1 ตุลาคม"
                    .into(),
                "กรณีปกติให้ยื่นและรออนุญาตก่อนหยุดงาน หากรอไม่ได้ให้ระบุเหตุจำเป็นและชี้แจงโดยเร็ว"
                    .into(),
                "หากมีเหตุพิเศษที่ยื่นล่วงหน้าไม่ได้ ให้ยื่นพร้อมเหตุผลในวันแรกที่กลับมาปฏิบัติงาน"
                    .into(),
                "ตรวจผู้มีอำนาจอนุญาตตามระยะลาต่อเนื่อง และตรวจสิทธิรับเงินเดือนแยกจากการอนุญาตลา"
                    .into(),
                FULL_DAY_WORKFLOW_GUIDANCE.into(),
            ],
        },
        LeavePolicyPreset {
            id: LeavePolicyPresetId::SchoolExit,
            name: "ออกนอกสถานศึกษา".into(),
            kind: LeaveKind::Exit,
            source_reference: None,
            source_url: SCHOOL_EXIT_EXAMPLE_URL.into(),
            personnel_types: Vec::new(),
            unit: LeavePolicyUnit::Minute,
            counting_mode: LeaveCountingMode::WorkingDays,
            quota_year_start_month: None,
            guidance: vec![
                "ใช้เวลาออกและเวลากลับภายในวันเดียวกัน"
                    .into(),
                "ระบุระเบียบของโรงเรียน กลุ่มบุคลากรที่ใช้เกณฑ์ และผู้อนุญาต กำหนดจำนวนนาทีสูงสุดต่อครั้งตามระเบียบของโรงเรียนถ้ามี"
                    .into(),
                "เอกสารที่แนบเป็นตัวอย่างแนวปฏิบัติจากโรงเรียนบ้านหนองลุมพุก ข้อ 2.6.6"
                    .into(),
            ],
        },
        LeavePolicyPreset {
            id: LeavePolicyPresetId::OfficialTravel,
            name: "เดินทางไปราชการในประเทศ (สพฐ.)".into(),
            kind: LeaveKind::OfficialTravel,
            source_reference: Some(
                "คำสั่งสำนักงานคณะกรรมการการศึกษาขั้นพื้นฐาน ที่ 1365/2560 ข้อ 3–5 เรื่อง มอบอำนาจอนุมัติการเดินทางไปราชการ"
                    .into(),
            ),
            source_url: DOMESTIC_TRAVEL_ORDER_URL.into(),
            personnel_types: vec![
                StaffPersonnelType::CivilServant,
                StaffPersonnelType::GovernmentEmployee,
                StaffPersonnelType::PermanentEmployee,
            ],
            unit: LeavePolicyUnit::Day,
            counting_mode: LeaveCountingMode::CalendarDays,
            quota_year_start_month: None,
            guidance: vec![
                "เลือกผู้อนุมัติตามต้นสังกัดและตรวจหลักฐานประกอบภารกิจ เช่น หนังสือเชิญหรือคำสั่ง"
                    .into(),
                "กรณีบุคลากรในโรงเรียนสังกัด สพฐ. เดินทางในประเทศ ให้เสนอผู้อำนวยการสถานศึกษา"
                    .into(),
                "หากผู้อำนวยการสถานศึกษาไปนอกจังหวัดหรือนอกเขตพื้นที่ตามคำสั่ง ให้เสนอผู้อำนวยการสำนักงานเขตพื้นที่การศึกษา กรณีอนุมัติการเดินทางของตนเองภายในพื้นที่ ให้รายงานเขตพื้นที่รายเดือน"
                    .into(),
            ],
        },
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_permission_codes as codes;
    use uuid::Uuid;

    #[test]
    fn policy_presets_require_school_configuration_access() {
        let actor = ActorContext {
            user_id: Uuid::new_v4(),
            permissions: vec![codes::STAFF_LEAVE_CREATE_OWN.into()],
        };
        assert!(matches!(
            list_policy_presets(&actor),
            Err(AppError::Forbidden(_))
        ));
    }

    #[test]
    fn policy_presets_offer_scoped_defaults_without_school_decisions() {
        let actor = ActorContext {
            user_id: Uuid::new_v4(),
            permissions: vec![codes::STAFF_LEAVE_MANAGE_SCHOOL.into()],
        };
        let presets = list_policy_presets(&actor).unwrap();
        assert_eq!(
            presets.iter().map(|item| item.id).collect::<Vec<_>>(),
            [
                LeavePolicyPresetId::SickLeave,
                LeavePolicyPresetId::PersonalLeave,
                LeavePolicyPresetId::SchoolExit,
                LeavePolicyPresetId::OfficialTravel,
            ]
        );
        for civil_service_leave in &presets[..2] {
            assert_eq!(civil_service_leave.kind, LeaveKind::Leave);
            assert_eq!(civil_service_leave.unit, LeavePolicyUnit::Day);
            assert_eq!(
                civil_service_leave.personnel_types,
                [StaffPersonnelType::CivilServant]
            );
            assert_eq!(
                civil_service_leave.counting_mode,
                LeaveCountingMode::WorkingDays
            );
            assert_eq!(civil_service_leave.quota_year_start_month, Some(10));
        }

        let exit = &presets[2];
        assert_eq!(exit.kind, LeaveKind::Exit);
        assert_eq!(exit.unit, LeavePolicyUnit::Minute);
        assert!(exit.source_reference.is_none());
        assert!(exit.personnel_types.is_empty());
        assert!(exit.quota_year_start_month.is_none());

        let travel = &presets[3];
        assert_eq!(travel.kind, LeaveKind::OfficialTravel);
        assert_eq!(travel.unit, LeavePolicyUnit::Day);
        assert_eq!(travel.counting_mode, LeaveCountingMode::CalendarDays);
        assert!(travel.quota_year_start_month.is_none());
    }
}
