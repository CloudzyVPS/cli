//! What every API command needs: settings, output format, a client.

use std::io::{BufRead, IsTerminal, Write};

use serde_json::Value;

use crate::api::{ApiClient, Request};
use crate::auth::session::Credential;
use crate::config::Settings;
use crate::error::{CliError, Result};
use crate::output::Format;

pub struct Ctx {
    pub settings: Settings,
    pub format: Format,
    pub debug: bool,
}

impl Ctx {
    pub fn api(&self) -> Result<ApiClient> {
        let http = super::auth::http_client();
        let credential = Credential::resolve(&self.settings, http.clone())?;
        Ok(ApiClient::new(http, &self.settings.url, credential).with_debug(self.debug))
    }

    pub async fn send(&self, req: Request) -> Result<Value> {
        Ok(self.api()?.send(req).await?)
    }

    pub fn json(&self) -> bool {
        self.format == Format::Json
    }

    /// Print a one-line success note on stderr, so stdout stays parseable.
    pub fn done(&self, message: impl AsRef<str>) {
        use yansi::Paint;
        eprintln!("{} {}", "✓".green(), message.as_ref());
    }
}

/// Ask before something irreversible. `--yes` skips the question; without a
/// terminal to ask on, the command refuses rather than guessing.
pub fn confirm(yes: bool, what: &str, expected: &str) -> Result<()> {
    if yes {
        return Ok(());
    }
    if !std::io::stdin().is_terminal() {
        return Err(CliError::Usage(format!(
            "{what} needs confirmation: pass --yes to run non-interactively"
        )));
    }
    eprint!("{what}. Type {expected} to confirm: ");
    let _ = std::io::stderr().flush();
    let mut line = String::new();
    std::io::stdin()
        .lock()
        .read_line(&mut line)
        .map_err(|e| CliError::Other(e.to_string()))?;
    if line.trim() == expected {
        Ok(())
    } else {
        Err(CliError::Usage(
            "confirmation did not match; nothing was changed".into(),
        ))
    }
}

/// Format cents as money: 1234 USD → "12.34 USD".
pub fn money(cents: Option<i64>, currency: Option<&str>) -> String {
    match cents {
        Some(c) => format!(
            "{}{}.{:02} {}",
            if c < 0 { "-" } else { "" },
            c.unsigned_abs() / 100,
            c.unsigned_abs() % 100,
            currency.unwrap_or("USD")
        ),
        None => "-".into(),
    }
}

/// API micro amounts are millionths of a currency unit. Keep all supplied
/// precision, trimming trailing zeros only down to the usual two places.
pub fn precise_money(micro: Option<i64>, cents: Option<i64>, currency: Option<&str>) -> String {
    let Some(value) = micro else {
        return money(cents, currency);
    };
    let magnitude = value.unsigned_abs();
    let mut fraction = format!("{:06}", magnitude % 1_000_000);
    while fraction.len() > 2 && fraction.ends_with('0') {
        fraction.pop();
    }
    format!(
        "{}{}.{} {}",
        if value < 0 { "-" } else { "" },
        magnitude / 1_000_000,
        fraction,
        currency.unwrap_or("USD")
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn precise_money() {
        assert_eq!(
            super::precise_money(Some(-11086), Some(0), None),
            "-0.011086 USD"
        );
        assert_eq!(super::precise_money(Some(1), Some(0), None), "0.000001 USD");
        assert_eq!(
            super::precise_money(Some(2500000), None, Some("EUR")),
            "2.50 EUR"
        );
        assert_eq!(super::precise_money(None, Some(-250), None), "-2.50 USD");
        assert_eq!(
            super::money(Some(i64::MIN), None),
            "-92233720368547758.08 USD"
        );
    }

    #[test]
    fn money() {
        assert_eq!(super::money(Some(1234), Some("EUR")), "12.34 EUR");
        assert_eq!(super::money(Some(-5), None), "-0.05 USD");
        assert_eq!(super::money(None, None), "-");
    }
}
