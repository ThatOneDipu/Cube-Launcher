use crate::auth;

pub mod elyby;
pub mod ms;

#[derive(Debug, Clone)]
pub struct AccountData {
    pub access_token: Option<String>,
    pub uuid: String,
    pub username: String,
    pub refresh_token: String,
    pub needs_refresh: bool,

    pub account_type: AccountType,
}

impl AccountData {
    pub fn get_username_modified(&self) -> String {
        let suffix = match self.account_type {
            auth::AccountType::Microsoft => "",
            auth::AccountType::ElyBy => " (elyby)",
        };
        format!("{}{suffix}", self.username)
    }
}

#[derive(Debug, Clone, Copy)]
pub enum AccountType {
    Microsoft,
    ElyBy,
}

impl std::fmt::Display for AccountType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                AccountType::Microsoft => "Microsoft",
                AccountType::ElyBy => "ElyBy",
            }
        )
    }
}

impl AccountData {
    #[must_use]
    pub fn is_elyby(&self) -> bool {
        let account_type = self.account_type;
        matches!(account_type, AccountType::ElyBy)
    }
}
