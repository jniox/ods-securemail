//! Composition SMTP : ce qui transforme `/verify` en verification.
//!
//! Le service garde les identifiants du serveur de messagerie d'un tenant. Dire qu'une
//! configuration est « verifiee » n'a de sens que si quelqu'un a compose le numero : ouvert
//! une connexion TCP, salue (EHLO), negocie TLS quand la configuration le demande, et
//! presente les identifiants dechiffres au serveur. C'est ce que fait [`LettreSmtpVerifier`].
//!
//! La composition est derriere un trait pour deux raisons : les tests de route ne doivent
//! pas dependre d'un serveur de messagerie, et le jour ou IMAP arrive il prendra le meme
//! chemin. Le trait est ecrit a la main (`Pin<Box<dyn Future>>`) plutot qu'avec `async fn`,
//! pour rester utilisable derriere un `Arc<dyn ...>` — meme forme que `EventProducer`
//! dans `ods-common`.

use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

use lettre::transport::smtp::authentication::Credentials;
use lettre::transport::smtp::client::{Tls, TlsParameters};
use lettre::transport::smtp::Error as LettreError;
use lettre::{AsyncSmtpTransport, Tokio1Executor};

/// Modes de chiffrement acceptes par la configuration (memes valeurs qu'en base).
pub const ENCRYPTION_TLS: &str = "tls";
pub const ENCRYPTION_STARTTLS: &str = "starttls";
pub const ENCRYPTION_NONE: &str = "none";

/// Delai d'attente par defaut d'une composition, en secondes.
///
/// Volontairement court : l'appel est synchrone du point de vue de l'appelant HTTP, et un
/// hote injoignable ne doit pas immobiliser un ouvrier pendant une minute.
pub const DEFAULT_TIMEOUT_SECS: u64 = 10;

/// Ce qu'il faut pour composer une fois. Le mot de passe est en clair, en memoire
/// seulement, le temps de l'appel.
#[derive(Clone)]
pub struct SmtpTarget {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub encryption: String,
}

/// `Debug` redige le mot de passe : une cible finit toujours par passer dans une trace.
impl fmt::Debug for SmtpTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SmtpTarget")
            .field("host", &self.host)
            .field("port", &self.port)
            .field("username", &self.username)
            .field("password", &"[redacted]")
            .field("encryption", &self.encryption)
            .finish()
    }
}

/// Pourquoi la verification a echoue, en quatre familles.
///
/// Les messages sont les NOTRES : on ne renvoie jamais au client le texte brut du serveur
/// distant, qui peut porter des banniores, des noms d'hotes internes ou des indices sur
/// les identifiants.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SmtpVerifyError {
    /// Rien au bout du fil : hote injoignable, port ferme, delai depasse, TLS impossible.
    Connect(String),
    /// Le serveur a parle, et il a refuse les identifiants.
    Auth(String),
    /// Le serveur a parle, et le dialogue a echoue pour une autre raison.
    Protocol(String),
    /// La configuration elle-meme est inutilisable : rien n'a ete compose.
    Config(String),
}

impl SmtpVerifyError {
    /// Etiquette lisible par machine, rendue dans la reponse de l'API.
    pub fn kind(&self) -> &'static str {
        match self {
            SmtpVerifyError::Connect(_) => "connect",
            SmtpVerifyError::Auth(_) => "auth",
            SmtpVerifyError::Protocol(_) => "protocol",
            SmtpVerifyError::Config(_) => "config",
        }
    }

    /// Message destine a l'utilisateur qui a saisi la configuration.
    pub fn message(&self) -> &str {
        match self {
            SmtpVerifyError::Connect(m)
            | SmtpVerifyError::Auth(m)
            | SmtpVerifyError::Protocol(m)
            | SmtpVerifyError::Config(m) => m,
        }
    }
}

impl fmt::Display for SmtpVerifyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.message())
    }
}

/// Composer un serveur SMTP et dire si les identifiants passent.
pub trait SmtpVerifier: Send + Sync {
    fn verify<'a>(
        &'a self,
        target: &'a SmtpTarget,
    ) -> Pin<Box<dyn Future<Output = Result<(), SmtpVerifyError>> + Send + 'a>>;
}

/// La composition reelle, sur `lettre`.
pub struct LettreSmtpVerifier {
    timeout: Duration,
}

impl LettreSmtpVerifier {
    pub fn new(timeout: Duration) -> Self {
        Self { timeout }
    }

    /// Traduit le mode de chiffrement de la configuration en politique TLS.
    fn tls_policy(target: &SmtpTarget) -> Result<Tls, SmtpVerifyError> {
        match target.encryption.as_str() {
            // Port 465 : TLS des la premiere octet, pas de phase en clair.
            ENCRYPTION_TLS => TlsParameters::new(target.host.clone())
                .map(Tls::Wrapper)
                .map_err(|_| {
                    SmtpVerifyError::Config(format!(
                        "TLS cannot be configured for host '{}'",
                        target.host
                    ))
                }),
            // Port 587 : on exige la bascule, on ne s'en remet pas a l'annonce du serveur.
            ENCRYPTION_STARTTLS => TlsParameters::new(target.host.clone())
                .map(Tls::Required)
                .map_err(|_| {
                    SmtpVerifyError::Config(format!(
                        "STARTTLS cannot be configured for host '{}'",
                        target.host
                    ))
                }),
            ENCRYPTION_NONE => Ok(Tls::None),
            other => Err(SmtpVerifyError::Config(format!(
                "Unsupported encryption mode '{other}'. Must be: tls, starttls, or none"
            ))),
        }
    }

    /// Range l'erreur de `lettre` dans une des quatre familles, sans recopier son texte.
    fn classify(err: &LettreError) -> SmtpVerifyError {
        if let Some(code) = err.status() {
            let code = code.to_string();
            return if is_authentication_code(&code) {
                SmtpVerifyError::Auth(
                    "The mail server rejected the credentials for this configuration".to_string(),
                )
            } else {
                SmtpVerifyError::Protocol(format!(
                    "The mail server refused the connection (SMTP {code})"
                ))
            };
        }
        if err.is_timeout() {
            return SmtpVerifyError::Connect(
                "The mail server did not answer before the timeout".to_string(),
            );
        }
        if err.is_tls() {
            return SmtpVerifyError::Connect(
                "TLS could not be negotiated with the mail server".to_string(),
            );
        }
        if err.is_response() {
            return SmtpVerifyError::Protocol(
                "The mail server sent a reply this client could not understand".to_string(),
            );
        }
        if err.is_client() {
            return SmtpVerifyError::Protocol(
                "The connection could not be completed with the mail server".to_string(),
            );
        }
        SmtpVerifyError::Connect("The mail server could not be reached".to_string())
    }
}

/// Les reponses SMTP qui veulent dire « pas avec ces identifiants-la ».
///
/// RFC 4954 : 432, 454, 534, 535, 538 ; RFC 5321 : 530 (authentification requise).
fn is_authentication_code(code: &str) -> bool {
    matches!(code, "432" | "454" | "530" | "534" | "535" | "538")
}

impl SmtpVerifier for LettreSmtpVerifier {
    fn verify<'a>(
        &'a self,
        target: &'a SmtpTarget,
    ) -> Pin<Box<dyn Future<Output = Result<(), SmtpVerifyError>> + Send + 'a>> {
        Box::pin(async move {
            let tls = Self::tls_policy(target)?;

            let transport: AsyncSmtpTransport<Tokio1Executor> =
                AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(target.host.clone())
                    .port(target.port)
                    .tls(tls)
                    .timeout(Some(self.timeout))
                    .credentials(Credentials::new(
                        target.username.clone(),
                        target.password.clone(),
                    ))
                    .build();

            // Le delai de `lettre` ne couvre QUE l'etablissement TCP : un serveur qui
            // accepte la connexion puis se tait laisserait la requete pendue. On borne
            // donc l'echange entier, pas seulement son premier pas.
            let outcome = tokio::time::timeout(self.timeout, transport.test_connection()).await;

            match outcome {
                Err(_elapsed) => Err(SmtpVerifyError::Connect(
                    "The mail server did not answer before the timeout".to_string(),
                )),
                Ok(Ok(true)) => Ok(()),
                Ok(Ok(false)) => Err(SmtpVerifyError::Protocol(
                    "The mail server closed the connection during the handshake".to_string(),
                )),
                Ok(Err(err)) => Err(Self::classify(&err)),
            }
        })
    }
}

/// Doublure a verdict fixe, pour les tests de route (qui ne peuvent pas atteindre un
/// `#[cfg(test)]` de la bibliotheque) et pour un demarrage local sans serveur de messagerie.
pub struct FixedSmtpVerifier {
    verdict: Result<(), SmtpVerifyError>,
}

impl FixedSmtpVerifier {
    pub fn accepting() -> Self {
        Self { verdict: Ok(()) }
    }

    pub fn rejecting(err: SmtpVerifyError) -> Self {
        Self { verdict: Err(err) }
    }
}

impl SmtpVerifier for FixedSmtpVerifier {
    fn verify<'a>(
        &'a self,
        _target: &'a SmtpTarget,
    ) -> Pin<Box<dyn Future<Output = Result<(), SmtpVerifyError>> + Send + 'a>> {
        let verdict = self.verdict.clone();
        Box::pin(async move { verdict })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_never_prints_the_password() {
        let target = SmtpTarget {
            host: "smtp.example.com".to_string(),
            port: 587,
            username: "postmaster@example.com".to_string(),
            password: "hunter2".to_string(),
            encryption: ENCRYPTION_STARTTLS.to_string(),
        };
        let printed = format!("{target:?}");
        assert!(!printed.contains("hunter2"), "printed: {printed}");
        assert!(printed.contains("[redacted]"));
    }

    #[test]
    fn every_supported_encryption_mode_maps_to_a_tls_policy() {
        let base = SmtpTarget {
            host: "smtp.example.com".to_string(),
            port: 587,
            username: "u".to_string(),
            password: "p".to_string(),
            encryption: ENCRYPTION_NONE.to_string(),
        };

        assert!(matches!(
            LettreSmtpVerifier::tls_policy(&base),
            Ok(Tls::None)
        ));

        let starttls = SmtpTarget {
            encryption: ENCRYPTION_STARTTLS.to_string(),
            ..base.clone()
        };
        assert!(matches!(
            LettreSmtpVerifier::tls_policy(&starttls),
            Ok(Tls::Required(_))
        ));

        let tls = SmtpTarget {
            encryption: ENCRYPTION_TLS.to_string(),
            ..base.clone()
        };
        assert!(matches!(
            LettreSmtpVerifier::tls_policy(&tls),
            Ok(Tls::Wrapper(_))
        ));

        let bogus = SmtpTarget {
            encryption: "ssl".to_string(),
            ..base
        };
        assert!(matches!(
            LettreSmtpVerifier::tls_policy(&bogus),
            Err(SmtpVerifyError::Config(_))
        ));
    }

    #[test]
    fn authentication_codes_are_told_apart_from_other_refusals() {
        for code in ["432", "454", "530", "534", "535", "538"] {
            assert!(is_authentication_code(code), "{code} is an AUTH refusal");
        }
        for code in ["421", "450", "550", "554", "250"] {
            assert!(!is_authentication_code(code), "{code} is not about AUTH");
        }
    }

    #[test]
    fn error_kinds_are_stable_tags() {
        assert_eq!(SmtpVerifyError::Connect("x".into()).kind(), "connect");
        assert_eq!(SmtpVerifyError::Auth("x".into()).kind(), "auth");
        assert_eq!(SmtpVerifyError::Protocol("x".into()).kind(), "protocol");
        assert_eq!(SmtpVerifyError::Config("x".into()).kind(), "config");
    }
}
