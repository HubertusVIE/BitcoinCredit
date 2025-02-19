// General
pub const BILLS_PREFIX: &str = "BILLS";
pub const BILL_PREFIX: &str = "BILL";
pub const BILL_ATTACHMENT_PREFIX: &str = "BILLATT";
pub const KEY_PREFIX: &str = "KEY";
pub const COMPANIES_PREFIX: &str = "COMPANIES";
pub const COMPANY_PREFIX: &str = "COMPANY";
pub const COMPANY_KEY_PREFIX: &str = "COMPANYKEY";
pub const COMPANY_CHAIN_PREFIX: &str = "COMPANYCHAIN";
pub const COMPANY_LOGO_PREFIX: &str = "COMPANYLOGO";
pub const COMPANY_PROOF_PREFIX: &str = "COMPANYPROOF";

// Currency
pub const SAT_TO_BTC_RATE: i64 = 100_000_000;

// Validation
pub const MAX_FILE_SIZE_BYTES: usize = 1_000_000; // ~1 MB
pub const MAX_FILE_NAME_CHARACTERS: usize = 200;
pub const VALID_FILE_MIME_TYPES: [&str; 3] = ["image/jpeg", "image/png", "application/pdf"];
