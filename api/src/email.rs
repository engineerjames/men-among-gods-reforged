//! SMTP-based email sender for password reset codes.
//!
//! Configured via environment variables:
//! - `SMTP_HOST` — SMTP server hostname (required for email support).
//! - `SMTP_PORT` — SMTP server port (default: `587`).
//! - `SMTP_USER` — SMTP authentication username.
//! - `SMTP_PASSWORD` — SMTP authentication password.
//! - `SMTP_FROM` — "From" address for outgoing emails.

use lettre::message::header::ContentType;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};
use log::{error, info};
use std::env;

/// Holds the SMTP transport and sender address used to dispatch password
/// reset emails.
#[derive(Clone)]
pub struct EmailSender {
    transport: AsyncSmtpTransport<Tokio1Executor>,
    from_address: String,
}

impl EmailSender {
    /// Attempts to build an `EmailSender` from environment variables.
    ///
    /// Returns `None` when `SMTP_HOST` is unset or empty — the API will
    /// start without email support and the reset endpoint will return 503.
    ///
    /// # Returns
    ///
    /// * `Some(EmailSender)` when SMTP is fully configured.
    /// * `None` when required env vars are missing.
    pub fn from_env() -> Option<Self> {
        let host = env::var("SMTP_HOST")
            .ok()
            .filter(|v| !v.trim().is_empty())?;
        let port: u16 = env::var("SMTP_PORT")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(587);
        let user = env::var("SMTP_USER").ok().unwrap_or_default();
        let password = env::var("SMTP_PASSWORD").ok().unwrap_or_default();
        let from = env::var("SMTP_FROM")
            .ok()
            .filter(|v| !v.trim().is_empty())
            .unwrap_or_else(|| format!("noreply@{host}"));

        let creds = Credentials::new(user, password);

        let transport = AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&host)
            .ok()?
            .port(port)
            .credentials(creds)
            .build();

        info!("SMTP email sender configured (host={host}, port={port})");
        Some(Self {
            transport,
            from_address: from,
        })
    }

    /// Sends a password reset code to the given email address.
    ///
    /// # Arguments
    ///
    /// * `to_email` - Recipient email address.
    /// * `code` - The 6-digit reset code to include in the message body.
    ///
    /// # Returns
    ///
    /// * `Ok(())` when the message was accepted by the SMTP server.
    /// * `Err(String)` on any send failure.
    pub async fn send_reset_code(&self, to_email: &str, code: &str) -> Result<(), String> {
        let email = Message::builder()
            .from(
                self.from_address
                    .parse()
                    .map_err(|e| format!("Invalid from address: {e}"))?,
            )
            .to(to_email
                .parse()
                .map_err(|e| format!("Invalid recipient address: {e}"))?)
            .subject("Men Among Gods — Password Reset Code")
            .header(ContentType::TEXT_PLAIN)
            .body(format!(
                "Your password reset code is: {code}\n\n\
                 This code expires in 15 minutes.\n\n\
                 If you did not request a password reset, you can safely ignore this email."
            ))
            .map_err(|e| format!("Failed to build email: {e}"))?;

        self.transport.send(email).await.map_err(|e| {
            error!("SMTP send failed: {e}");
            format!("Failed to send email: {e}")
        })?;

        info!("Password reset code sent to {to_email}");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_env_returns_none_without_smtp_host() {
        // Ensure SMTP_HOST is not set.
        std::env::remove_var("SMTP_HOST");
        assert!(EmailSender::from_env().is_none());
    }
}
