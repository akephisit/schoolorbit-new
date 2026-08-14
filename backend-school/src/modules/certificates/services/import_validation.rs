use std::collections::{BTreeMap, BTreeSet};

use unicode_normalization::UnicodeNormalization;

use crate::modules::certificates::models::{
    CertificateImportRequest, CertificateImportRowInput, CertificateImportSource, RecipientType,
};

pub const MAX_IMPORT_ROWS: usize = 5_000;
pub const MAX_CUSTOM_COLUMNS: usize = 64;
pub const MAX_HEADER_SCALARS: usize = 100;
pub const MAX_NAME_SCALARS: usize = 100;
pub const MAX_CUSTOM_VALUE_SCALARS: usize = 500;

pub const RENDERABLE_STANDARD_VARIABLES: [&str; 5] =
    ["คำนำหน้า", "ชื่อ", "นามสกุล", "รายการกิจกรรม", "รางวัลหรือบทบาท"];

pub const RESERVED_RENDER_VARIABLES: [&str; 8] = [
    "ปีการศึกษา",
    "ชื่อกิจกรรมหลัก",
    "เลขเกียรติบัตร",
    "วันที่จัดกิจกรรม",
    "วันที่ออก",
    "ชื่อโรงเรียนผู้ออก",
    "ชื่อหน่วยงานเจ้าของกิจกรรม",
    "QR_CODE",
];

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum StandardColumn {
    RecipientType,
    StudentId,
    StaffUsername,
    Title,
    FirstName,
    LastName,
    ActivityItem,
    AwardOrRole,
    TemplateName,
}

impl StandardColumn {
    pub const ALL: [Self; 9] = [
        Self::RecipientType,
        Self::StudentId,
        Self::StaffUsername,
        Self::Title,
        Self::FirstName,
        Self::LastName,
        Self::ActivityItem,
        Self::AwardOrRole,
        Self::TemplateName,
    ];

    pub const REQUIRED: [Self; 3] = [Self::RecipientType, Self::FirstName, Self::LastName];

    pub const fn header(self) -> &'static str {
        match self {
            Self::RecipientType => "ประเภทผู้รับ",
            Self::StudentId => "รหัสนักเรียน",
            Self::StaffUsername => "ชื่อผู้ใช้บุคลากร",
            Self::Title => "คำนำหน้า",
            Self::FirstName => "ชื่อ",
            Self::LastName => "นามสกุล",
            Self::ActivityItem => "รายการกิจกรรม",
            Self::AwardOrRole => "รางวัลหรือบทบาท",
            Self::TemplateName => "แบบเกียรติบัตร",
        }
    }

    pub const fn is_renderable(self) -> bool {
        matches!(
            self,
            Self::Title | Self::FirstName | Self::LastName | Self::ActivityItem | Self::AwardOrRole
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HeaderClass {
    Standard(StandardColumn),
    ReservedSystemVariable,
    Forbidden,
    Custom(String),
    Invalid,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImportHeaderError {
    NoRows,
    TooManyRows,
    EmptyHeader,
    HeaderTooLong,
    DuplicateHeader,
    ForbiddenHeader,
    ReservedHeader,
    MissingRequired(StandardColumn),
    TooManyCustomColumns,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidatedImportHeaders {
    pub normalized_headers: Vec<String>,
    pub custom_headers: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImportRequestError {
    InvalidSource,
    Headers(ImportHeaderError),
    UnknownCustomColumn,
    DuplicateCustomColumn,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ImportRowIssue {
    InvalidRecipientType,
    MissingStudentId,
    MissingStaffUsername,
    UnexpectedInternalLookup,
    MissingFirstName,
    MissingLastName,
    NameTooLong,
    ValueTooLong,
    ForbiddenSensitiveValue,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImportRowValidationOutcome {
    pub recipient_type: Option<RecipientType>,
    pub issues: Vec<ImportRowIssue>,
}

impl ImportRowValidationOutcome {
    pub fn is_valid(&self) -> bool {
        self.issues.is_empty()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VariableInterpolationError {
    InvalidSyntax,
    UnknownVariable(String),
}

pub fn classify_header(header: &str) -> HeaderClass {
    let display = normalize_display_text(header);
    if display.is_empty() || display.chars().count() > MAX_HEADER_SCALARS {
        return HeaderClass::Invalid;
    }
    let key = normalized_comparison_key(&display);
    if is_forbidden_header(&key) {
        return HeaderClass::Forbidden;
    }
    if RESERVED_RENDER_VARIABLES
        .iter()
        .any(|reserved| normalized_comparison_key(reserved) == key)
    {
        return HeaderClass::ReservedSystemVariable;
    }
    if let Some(column) = StandardColumn::ALL
        .into_iter()
        .find(|column| normalized_comparison_key(column.header()) == key)
    {
        return HeaderClass::Standard(column);
    }
    HeaderClass::Custom(display)
}

pub fn validate_import_headers(
    headers: &[String],
    row_count: usize,
) -> Result<ValidatedImportHeaders, ImportHeaderError> {
    if row_count == 0 {
        return Err(ImportHeaderError::NoRows);
    }
    if row_count > MAX_IMPORT_ROWS {
        return Err(ImportHeaderError::TooManyRows);
    }

    let mut keys = BTreeSet::new();
    let mut standard = BTreeSet::new();
    let mut normalized_headers = Vec::with_capacity(headers.len());
    let mut custom_headers = Vec::new();
    for raw_header in headers {
        let display = normalize_display_text(raw_header);
        if display.is_empty() {
            return Err(ImportHeaderError::EmptyHeader);
        }
        if display.chars().count() > MAX_HEADER_SCALARS {
            return Err(ImportHeaderError::HeaderTooLong);
        }
        if !keys.insert(normalized_comparison_key(&display)) {
            return Err(ImportHeaderError::DuplicateHeader);
        }
        match classify_header(&display) {
            HeaderClass::Standard(column) => {
                standard.insert(column);
            }
            HeaderClass::ReservedSystemVariable => {
                return Err(ImportHeaderError::ReservedHeader);
            }
            HeaderClass::Forbidden => return Err(ImportHeaderError::ForbiddenHeader),
            HeaderClass::Custom(header) => custom_headers.push(header),
            HeaderClass::Invalid => return Err(ImportHeaderError::EmptyHeader),
        }
        normalized_headers.push(display);
    }

    for required in StandardColumn::REQUIRED {
        if !standard.contains(&required) {
            return Err(ImportHeaderError::MissingRequired(required));
        }
    }
    if custom_headers.len() > MAX_CUSTOM_COLUMNS {
        return Err(ImportHeaderError::TooManyCustomColumns);
    }

    Ok(ValidatedImportHeaders {
        normalized_headers,
        custom_headers,
    })
}

pub fn validate_import_request(
    request: &CertificateImportRequest,
) -> Result<ValidatedImportHeaders, ImportRequestError> {
    if !matches!(
        request.source,
        CertificateImportSource::Xlsx | CertificateImportSource::Csv
    ) {
        return Err(ImportRequestError::InvalidSource);
    }
    let headers = validate_import_headers(&request.headers, request.rows.len())
        .map_err(ImportRequestError::Headers)?;
    let custom_keys = headers
        .custom_headers
        .iter()
        .map(|header| normalized_comparison_key(header))
        .collect::<BTreeSet<_>>();
    for row in &request.rows {
        let normalized_row_keys = row
            .custom_values
            .keys()
            .map(|header| normalized_comparison_key(header))
            .collect::<Vec<_>>();
        if normalized_row_keys
            .iter()
            .any(|header| !custom_keys.contains(header))
        {
            return Err(ImportRequestError::UnknownCustomColumn);
        }
        if normalized_row_keys.iter().collect::<BTreeSet<_>>().len() != normalized_row_keys.len() {
            return Err(ImportRequestError::DuplicateCustomColumn);
        }
    }
    Ok(headers)
}

pub fn validate_import_row(row: &CertificateImportRowInput) -> ImportRowValidationOutcome {
    let recipient_type = parse_recipient_type(&row.recipient_type);
    let mut issues = Vec::new();
    let student_id_present = has_text(row.student_id.as_deref());
    let staff_username_present = has_text(row.staff_username.as_deref());
    match recipient_type {
        Some(RecipientType::Student) => {
            if !student_id_present {
                issues.push(ImportRowIssue::MissingStudentId);
            }
            if staff_username_present {
                issues.push(ImportRowIssue::UnexpectedInternalLookup);
            }
        }
        Some(RecipientType::Staff) => {
            if !staff_username_present {
                issues.push(ImportRowIssue::MissingStaffUsername);
            }
            if student_id_present {
                issues.push(ImportRowIssue::UnexpectedInternalLookup);
            }
        }
        Some(RecipientType::External) => {
            if student_id_present || staff_username_present {
                issues.push(ImportRowIssue::UnexpectedInternalLookup);
            }
        }
        None => issues.push(ImportRowIssue::InvalidRecipientType),
    }

    if row.first_name.trim().is_empty() {
        issues.push(ImportRowIssue::MissingFirstName);
    }
    if row.last_name.trim().is_empty() {
        issues.push(ImportRowIssue::MissingLastName);
    }
    if [
        Some(row.first_name.as_str()),
        Some(row.last_name.as_str()),
        row.title.as_deref(),
    ]
    .into_iter()
    .flatten()
    .any(|value| value.chars().count() > MAX_NAME_SCALARS)
    {
        issues.push(ImportRowIssue::NameTooLong);
    }
    if [row.activity_item.as_deref(), row.award_or_role.as_deref()]
        .into_iter()
        .flatten()
        .chain(row.custom_values.values().map(String::as_str))
        .any(|value| value.chars().count() > MAX_CUSTOM_VALUE_SCALARS)
    {
        issues.push(ImportRowIssue::ValueTooLong);
    }
    if [
        Some(row.recipient_type.as_str()),
        row.student_id.as_deref(),
        row.staff_username.as_deref(),
        row.title.as_deref(),
        Some(row.first_name.as_str()),
        Some(row.last_name.as_str()),
        row.activity_item.as_deref(),
        row.award_or_role.as_deref(),
        row.template_name.as_deref(),
    ]
    .into_iter()
    .flatten()
    .chain(row.custom_values.values().map(String::as_str))
    .any(contains_thirteen_digit_run)
    {
        issues.push(ImportRowIssue::ForbiddenSensitiveValue);
    }
    if row.recipient_type.chars().count() > 50
        || row
            .student_id
            .as_ref()
            .is_some_and(|value| value.chars().count() > 50)
        || row
            .staff_username
            .as_ref()
            .is_some_and(|value| value.chars().count() > 100)
        || row
            .template_name
            .as_ref()
            .is_some_and(|value| value.chars().count() > 200)
    {
        issues.push(ImportRowIssue::ValueTooLong);
    }

    issues.sort_unstable();
    issues.dedup();
    ImportRowValidationOutcome {
        recipient_type,
        issues,
    }
}

pub fn parse_recipient_type(value: &str) -> Option<RecipientType> {
    match normalized_comparison_key(value).as_str() {
        "student" | "นักเรียน" => Some(RecipientType::Student),
        "staff" | "บุคลากร" => Some(RecipientType::Staff),
        "external" | "บุคคลภายนอก" => Some(RecipientType::External),
        _ => None,
    }
}

pub fn recipient_type_is_allowed(
    recipient_type: RecipientType,
    allowed_recipient_types: &[RecipientType],
) -> bool {
    allowed_recipient_types.contains(&recipient_type)
}

pub fn normalize_name_for_match(value: &str) -> String {
    normalized_comparison_key(value)
}

pub fn normalize_template_name(value: &str) -> String {
    normalized_comparison_key(value)
}

pub fn variable_catalog(custom_headers: &[String]) -> Result<Vec<String>, ImportHeaderError> {
    let required_headers = StandardColumn::REQUIRED
        .into_iter()
        .map(|column| column.header().to_string())
        .chain(custom_headers.iter().cloned())
        .collect::<Vec<_>>();
    let validated = validate_import_headers(&required_headers, 1)?;
    let mut catalog = RENDERABLE_STANDARD_VARIABLES
        .into_iter()
        .chain(RESERVED_RENDER_VARIABLES)
        .map(str::to_string)
        .collect::<Vec<_>>();
    catalog.extend(validated.custom_headers);
    Ok(catalog)
}

pub fn referenced_variables(content: &str) -> Result<Vec<String>, VariableInterpolationError> {
    let mut variables = Vec::new();
    let mut cursor = 0;
    while let Some(relative_start) = content[cursor..].find('{') {
        let start = cursor + relative_start;
        if content[cursor..start].contains('}') {
            return Err(VariableInterpolationError::InvalidSyntax);
        }
        let value_start = start + 1;
        let Some(relative_end) = content[value_start..].find('}') else {
            return Err(VariableInterpolationError::InvalidSyntax);
        };
        let end = value_start + relative_end;
        let variable = normalize_display_text(&content[value_start..end]);
        if variable.is_empty() || variable.contains(['{', '}']) {
            return Err(VariableInterpolationError::InvalidSyntax);
        }
        variables.push(variable);
        cursor = end + 1;
    }
    if content[cursor..].contains('}') {
        return Err(VariableInterpolationError::InvalidSyntax);
    }
    Ok(variables)
}

pub fn interpolate_plain_text(
    content: &str,
    values: &BTreeMap<String, String>,
) -> Result<String, VariableInterpolationError> {
    let normalized_values = values
        .iter()
        .map(|(key, value)| (normalized_comparison_key(key), value))
        .collect::<BTreeMap<_, _>>();
    let mut output = String::with_capacity(content.len());
    let mut cursor = 0;
    while let Some(relative_start) = content[cursor..].find('{') {
        let start = cursor + relative_start;
        if content[cursor..start].contains('}') {
            return Err(VariableInterpolationError::InvalidSyntax);
        }
        output.push_str(&content[cursor..start]);
        let value_start = start + 1;
        let Some(relative_end) = content[value_start..].find('}') else {
            return Err(VariableInterpolationError::InvalidSyntax);
        };
        let end = value_start + relative_end;
        let display = normalize_display_text(&content[value_start..end]);
        if display.is_empty() || display.contains(['{', '}']) {
            return Err(VariableInterpolationError::InvalidSyntax);
        }
        let value = normalized_values
            .get(&normalized_comparison_key(&display))
            .ok_or_else(|| VariableInterpolationError::UnknownVariable(display.clone()))?;
        output.push_str(value);
        cursor = end + 1;
    }
    if content[cursor..].contains('}') {
        return Err(VariableInterpolationError::InvalidSyntax);
    }
    output.push_str(&content[cursor..]);
    Ok(output)
}

pub fn normalize_display_text(value: &str) -> String {
    value
        .nfc()
        .filter(|character| !matches!(character, '\u{200b}' | '\u{200c}' | '\u{200d}' | '\u{feff}'))
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn normalized_comparison_key(value: &str) -> String {
    normalize_display_text(value)
        .chars()
        .flat_map(char::to_lowercase)
        .collect::<String>()
        .nfc()
        .collect()
}

pub(super) fn is_forbidden_header(normalized_header: &str) -> bool {
    let compact = normalized_comparison_key(normalized_header)
        .chars()
        .filter(|character| character.is_alphanumeric())
        .collect::<String>();
    [
        "nationalid",
        "citizenid",
        "citizenshipid",
        "เลขบัตรประชาชน",
        "บัตรประจำตัวประชาชน",
        "เลขบัตรประจำตัวประชาชน",
        "เลขประจำตัวประชาชน",
        "หมายเลขบัตรประชาชน",
    ]
    .into_iter()
    .any(|forbidden| compact.contains(forbidden))
}

fn has_text(value: Option<&str>) -> bool {
    value.is_some_and(|value| !value.trim().is_empty())
}

pub(super) fn contains_thirteen_digit_run(value: &str) -> bool {
    let mut digits = 0_u8;
    for character in normalize_display_text(value).chars() {
        if character.is_numeric() {
            digits = digits.saturating_add(1);
            if digits >= 13 {
                return true;
            }
        } else if digits > 0 && !character.is_alphanumeric() {
            continue;
        } else {
            digits = 0;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use crate::modules::certificates::{
        models::{
            CertificateImportRequest, CertificateImportRowInput, CertificateImportSource,
            RecipientType,
        },
        services::import_validation::{
            classify_header, interpolate_plain_text, normalize_name_for_match,
            recipient_type_is_allowed, validate_import_headers, validate_import_request,
            validate_import_row, variable_catalog, HeaderClass, ImportHeaderError,
            ImportRequestError, ImportRowIssue, StandardColumn,
        },
    };

    #[test]
    fn rejects_forbidden_and_reserved_headers_after_unicode_normalization() {
        assert_eq!(classify_header(" national_id "), HeaderClass::Forbidden);
        assert_eq!(classify_header("เลขประจำตัวประชาชน"), HeaderClass::Forbidden);
        assert_eq!(
            classify_header("ชื่อ"),
            HeaderClass::Standard(StandardColumn::FirstName)
        );
        assert_eq!(
            classify_header("ชื่อโรงเรียนผู้ออก"),
            HeaderClass::ReservedSystemVariable
        );
        assert_eq!(
            classify_header("ครูผู้ควบคุม"),
            HeaderClass::Custom("ครูผู้ควบคุม".into())
        );
    }

    #[test]
    fn validates_required_duplicate_and_capacity_limits_atomically() {
        let required = vec!["ประเภทผู้รับ".into(), "ชื่อ".into(), "นามสกุล".into()];
        assert!(validate_import_headers(&required, 5_000).is_ok());
        assert_eq!(
            validate_import_headers(&required, 5_001),
            Err(ImportHeaderError::TooManyRows)
        );
        assert_eq!(
            validate_import_headers(&["ประเภทผู้รับ".into(), "ชื่อ".into()], 1),
            Err(ImportHeaderError::MissingRequired(StandardColumn::LastName))
        );
        assert!(matches!(
            validate_import_headers(
                &[
                    "ประเภทผู้รับ".into(),
                    "ชื่อ".into(),
                    " ชื่อ ".into(),
                    "นามสกุล".into()
                ],
                1,
            ),
            Err(ImportHeaderError::DuplicateHeader)
        ));

        let mut too_many_custom = required;
        too_many_custom.extend((0..65).map(|index| format!("คอลัมน์ {index}")));
        assert_eq!(
            validate_import_headers(&too_many_custom, 1),
            Err(ImportHeaderError::TooManyCustomColumns)
        );

        let exactly_allowed = vec!["ประเภทผู้รับ".into(), "ชื่อ".into(), "นามสกุล".into()]
            .into_iter()
            .chain((0..64).map(|index| format!("ตัวแปร {index}")))
            .collect::<Vec<_>>();
        assert_eq!(
            validate_import_headers(&exactly_allowed, 1)
                .unwrap()
                .custom_headers
                .len(),
            64
        );
        assert_eq!(
            validate_import_headers(
                &[
                    "ประเภทผู้รับ".into(),
                    "ชื่อ".into(),
                    "นามสกุล".into(),
                    "Custom Field".into(),
                    "custom   field".into(),
                ],
                1,
            ),
            Err(ImportHeaderError::DuplicateHeader)
        );
    }

    #[test]
    fn normalizes_names_and_interpolates_only_known_plain_text_variables() {
        assert_eq!(
            normalize_name_for_match("  JOSE\u{301}   SMITH "),
            normalize_name_for_match("josé smith")
        );

        let values = BTreeMap::from([
            ("ชื่อ".to_string(), "<กมล>".to_string()),
            ("รางวัลหรือบทบาท".to_string(), "วิทยากร".to_string()),
        ]);
        assert_eq!(
            interpolate_plain_text("มอบให้ {ชื่อ} ในฐานะ {รางวัลหรือบทบาท}", &values).unwrap(),
            "มอบให้ <กมล> ในฐานะ วิทยากร"
        );
        assert!(interpolate_plain_text("{ตัวแปรที่ไม่มี}", &values).is_err());
        assert!(interpolate_plain_text("วงเล็บเกิน } {ชื่อ}", &values).is_err());
    }

    #[test]
    fn recipient_template_compatibility_is_exact_for_every_type() {
        let all = [
            RecipientType::Student,
            RecipientType::Staff,
            RecipientType::External,
        ];
        for recipient in all {
            for allowed in all {
                assert_eq!(
                    recipient_type_is_allowed(recipient, &[allowed]),
                    recipient == allowed
                );
            }
        }
        assert!(recipient_type_is_allowed(
            RecipientType::External,
            &[RecipientType::Student, RecipientType::External]
        ));
    }

    fn external_row() -> CertificateImportRowInput {
        CertificateImportRowInput {
            recipient_type: "บุคคลภายนอก".into(),
            student_id: None,
            staff_username: None,
            title: None,
            first_name: "กมล".into(),
            last_name: "ใจดี".into(),
            activity_item: None,
            award_or_role: Some("วิทยากร".into()),
            template_name: None,
            custom_values: BTreeMap::new(),
        }
    }

    #[test]
    fn validates_typed_request_source_custom_keys_and_row_lengths() {
        let mut request = CertificateImportRequest {
            source: CertificateImportSource::Csv,
            headers: vec![
                "ประเภทผู้รับ".into(),
                "ชื่อ".into(),
                "นามสกุล".into(),
                "ครูผู้ควบคุม".into(),
            ],
            rows: vec![external_row()],
        };
        request.rows[0]
            .custom_values
            .insert("ครูผู้ควบคุม".into(), "ครูตัวอย่าง".into());
        assert!(validate_import_request(&request).is_ok());

        request.source = CertificateImportSource::Manual;
        assert_eq!(
            validate_import_request(&request),
            Err(ImportRequestError::InvalidSource)
        );
        request.source = CertificateImportSource::Csv;
        request.rows[0]
            .custom_values
            .insert("คอลัมน์ที่ไม่ได้ประกาศ".into(), "ค่า".into());
        assert_eq!(
            validate_import_request(&request),
            Err(ImportRequestError::UnknownCustomColumn)
        );

        let mut invalid_student = external_row();
        invalid_student.recipient_type = "student".into();
        invalid_student.first_name = "ก".repeat(101);
        let outcome = validate_import_row(&invalid_student);
        assert!(outcome.issues.contains(&ImportRowIssue::MissingStudentId));
        assert!(outcome.issues.contains(&ImportRowIssue::NameTooLong));

        let mut invalid_external = external_row();
        invalid_external.student_id = Some("S-EXAMPLE".into());
        invalid_external
            .custom_values
            .insert("หมายเหตุ".into(), "ก".repeat(501));
        let outcome = validate_import_row(&invalid_external);
        assert!(outcome
            .issues
            .contains(&ImportRowIssue::UnexpectedInternalLookup));
        assert!(outcome.issues.contains(&ImportRowIssue::ValueTooLong));

        invalid_external
            .custom_values
            .insert("หมายเหตุ".into(), "ข้อมูลต้องห้าม 0-0000-00000-00-0".into());
        assert!(validate_import_row(&invalid_external)
            .issues
            .contains(&ImportRowIssue::ForbiddenSensitiveValue));
    }

    #[test]
    fn request_contract_denies_unknown_json_fields_and_catalog_excludes_controls() {
        let unexpected = serde_json::json!({
            "source": "csv",
            "headers": ["ประเภทผู้รับ", "ชื่อ", "นามสกุล"],
            "rows": [{
                "recipientType": "external",
                "firstName": "กมล",
                "lastName": "ใจดี",
                "unexpected": "must fail"
            }]
        });
        assert!(serde_json::from_value::<CertificateImportRequest>(unexpected).is_err());

        let catalog = variable_catalog(&["ครูผู้ควบคุม".into()]).unwrap();
        assert!(catalog.contains(&"ชื่อ".to_string()));
        assert!(catalog.contains(&"ครูผู้ควบคุม".to_string()));
        assert!(catalog.contains(&"QR_CODE".to_string()));
        assert!(!catalog.contains(&"รหัสนักเรียน".to_string()));
        assert!(!catalog.contains(&"ชื่อผู้ใช้บุคลากร".to_string()));
        assert!(!catalog.contains(&"แบบเกียรติบัตร".to_string()));
    }

    #[test]
    fn forbidden_header_detection_cannot_be_bypassed_with_separators() {
        for header in [
            "NATIONAL-ID",
            "citizen id",
            "เลขบัตรประจำตัวประชาชน",
            "เลข\u{200b}ประจำตัวประชาชน",
        ] {
            assert_eq!(classify_header(header), HeaderClass::Forbidden);
        }
    }

    #[test]
    fn sensitive_value_detection_covers_unicode_digits_and_invisible_characters() {
        for sensitive_value in [
            "ข้อมูล 1\u{200b}-2345-67890-12-3",
            "ข้อมูล ๑-๒๓๔๕-๖๗๘๙๐-๑๒-๓",
            "ข้อมูล １-２３４５-６７８９０-１２-３",
            "ข้อมูล １－２３４５－６７８９０－１２－３",
        ] {
            let mut row = external_row();
            row.award_or_role = Some(sensitive_value.into());

            assert!(
                validate_import_row(&row)
                    .issues
                    .contains(&ImportRowIssue::ForbiddenSensitiveValue),
                "sensitive value bypassed validation: {sensitive_value}"
            );
        }
    }
}
