use anyhow::{Context, Result};
use keyring::Entry;

const SERVICE_NAME: &str = "forgetmeifyoucan";
const USERNAME: &str = "jwt_token";

/// Store JWT token securely in system keyring
pub fn store_token(token: &str) -> Result<()> {
    let entry = Entry::new(SERVICE_NAME, USERNAME)
        .context("Failed to access system keyring")?;

    entry.set_password(token)
        .context("Failed to store token in keyring")?;

    Ok(())
}

/// Retrieve JWT token from system keyring
pub fn get_token() -> Result<String> {
    let entry = Entry::new(SERVICE_NAME, USERNAME)
        .context("Failed to access system keyring")?;

    entry.get_password()
        .context("No token found. Please run 'zen login' first")
}

/// Delete JWT token from system keyring
pub fn delete_token() -> Result<()> {
    let entry = Entry::new(SERVICE_NAME, USERNAME)
        .context("Failed to access system keyring")?;

    entry.delete_credential()
        .context("Failed to delete token from keyring")?;

    Ok(())
}

/// Check if user is logged in
pub fn is_logged_in() -> bool {
    get_token().is_ok()
}
