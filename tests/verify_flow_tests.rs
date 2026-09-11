//! Le chemin complet d'une verification : base → dechiffrement → socket → verdict persiste.
//!
//! Les tests de route prouvent que le point d'entree rend le verdict du composeur ; les
//! tests du composeur prouvent qu'il dialogue vraiment. Il restait le maillon du milieu :
//! est-ce que le mot de passe CHIFFRE EN BASE ressort intact au bout du fil ?
//!
//! Ce test fait le trajet entier avec le composeur REEL (`LettreSmtpVerifier`) : une
//! configuration est ecrite en base par le depot (mot de passe chiffre AES-256-GCM),
//! `MailConfigService::verify` la relit, la dechiffre et compose le faux serveur, qui
//! confirme avoir recu ce mot de passe-la. C'est l'activite de validation « Phase 1 »
//! du brief GTM : « configure a real SMTP endpoint; call verify; confirm TCP handshake
//! and AUTH exchange ».
//!
//! Le port du faux serveur est ephemere, donc hors de la liste metier (25/465/587) que
//! `create()` impose : on passe par le depot, qui est la couche sous cette validation.

mod common;

use std::sync::Arc;
use std::time::Duration;

use base64::Engine as _;
use uuid::Uuid;

use common::{AuthOutcome, FakeSmtpServer};
use ods_securemail::domain::mail_config::{CreateMailConfig, UpdateMailConfig};
use ods_securemail::events::producer::InMemoryProducer;
use ods_securemail::repository::{db, mail_config::MailConfigRepository};
use ods_securemail::service::mail_config_service::MailConfigService;
use ods_securemail::service::smtp_verifier::{LettreSmtpVerifier, SmtpVerifier};

const SMTP_PASSWORD: &str = "correct-horse-battery-staple";

async fn service_with_real_dialer() -> (MailConfigService, Arc<MailConfigRepository>) {
    let _ = dotenvy::dotenv();
    let database_url =
        std::env::var("DATABASE_URL").expect("DATABASE_URL must be set for integration tests");

    let pool = db::create_pool(&database_url, "securemail", 5)
        .await
        .expect("Failed to create pool");
    db::run_migrations(&pool)
        .await
        .expect("Failed to run securemail migrations");

    let repo = Arc::new(MailConfigRepository::new(pool));
    let verifier: Arc<dyn SmtpVerifier> = Arc::new(LettreSmtpVerifier::new(Duration::from_secs(5)));

    let svc = MailConfigService::new(
        repo.clone(),
        Arc::new(InMemoryProducer::new()) as Arc<dyn ods_common::events::EventProducer>,
        vec![0x42u8; 32],
        verifier,
    );
    (svc, repo)
}

/// Ecrit une configuration dont le serveur SMTP est le faux serveur du test.
///
/// Deux temps, parce que la validation metier n'accepte que les ports 25/465/587 et que
/// le faux serveur ecoute sur un port ephemere : le SERVICE cree la configuration (c'est
/// lui qui chiffre le mot de passe — ce qu'on veut prouver), puis le DEPOT repointe le
/// port, une couche sous cette validation. Rien n'est contourne du chemin teste.
async fn store_config_pointing_at(
    svc: &MailConfigService,
    repo: &MailConfigRepository,
    tenant_id: Uuid,
    port: u16,
    name: &str,
) -> Uuid {
    let created = svc
        .create(tenant_id, config_template(name))
        .await
        .expect("create");

    repo.update(
        tenant_id,
        created.id,
        &UpdateMailConfig {
            name: None,
            smtp_host: None,
            smtp_port: Some(i32::from(port)),
            smtp_username: None,
            smtp_password: None,
            smtp_encryption: None,
            imap_host: None,
            imap_port: None,
            imap_username: None,
            imap_password: None,
            imap_encryption: None,
            from_address: None,
            from_name: None,
            is_default: None,
        },
        None,
        None,
    )
    .await
    .expect("repoint the config at the fake server");

    created.id
}

fn config_template(name: &str) -> CreateMailConfig {
    CreateMailConfig {
        name: name.to_string(),
        smtp_host: "127.0.0.1".to_string(),
        smtp_port: 587,
        smtp_username: "postmaster@example.com".to_string(),
        smtp_password: SMTP_PASSWORD.to_string(),
        smtp_encryption: "none".to_string(),
        imap_host: None,
        imap_port: None,
        imap_username: None,
        imap_password: None,
        imap_encryption: None,
        from_address: "noreply@example.com".to_string(),
        from_name: "SecureMail end-to-end".to_string(),
        is_default: false,
    }
}

#[tokio::test]
async fn the_stored_password_is_the_one_presented_to_the_mail_server() {
    let server = FakeSmtpServer::start(AuthOutcome::Accept).await;
    let (svc, repo) = service_with_real_dialer().await;
    let tenant_id = Uuid::new_v4();

    // Ecriture par le service : le mot de passe part chiffre en base.
    let config_id = store_config_pointing_at(
        &svc,
        &repo,
        tenant_id,
        server.addr.port(),
        &format!("End-to-end verify {tenant_id}"),
    )
    .await;

    let outcome = svc.verify(tenant_id, config_id).await.expect("verify");
    assert!(
        outcome.verified(),
        "the dial should succeed, error: {:?}",
        outcome.error
    );

    // Le drapeau est bien alle en base, pas seulement dans la reponse.
    let reread = repo.get_by_id(tenant_id, config_id).await.expect("reread");
    assert!(reread.verified, "the verdict must be persisted");

    // Et le serveur a bien recu le mot de passe d'origine, apres un aller-retour
    // chiffrement AES-256-GCM -> PostgreSQL -> dechiffrement.
    let commands = server.commands();
    let auth = commands
        .iter()
        .find(|c| c.to_ascii_uppercase().starts_with("AUTH"))
        .unwrap_or_else(|| panic!("no AUTH in transcript: {commands:?}"));
    let payload = auth
        .split_whitespace()
        .nth(2)
        .expect("AUTH PLAIN <payload>");
    let decoded = String::from_utf8(
        base64::engine::general_purpose::STANDARD
            .decode(payload)
            .expect("base64"),
    )
    .expect("utf-8");
    assert!(
        decoded.contains(SMTP_PASSWORD),
        "the mail server must receive the decrypted password"
    );
}

#[tokio::test]
async fn a_server_that_refuses_the_credentials_leaves_the_config_unverified() {
    let server = FakeSmtpServer::start(AuthOutcome::Reject).await;
    let (svc, repo) = service_with_real_dialer().await;
    let tenant_id = Uuid::new_v4();

    let config_id = store_config_pointing_at(
        &svc,
        &repo,
        tenant_id,
        server.addr.port(),
        &format!("End-to-end refusal {tenant_id}"),
    )
    .await;

    let outcome = svc.verify(tenant_id, config_id).await.expect("verify");
    assert!(!outcome.verified());
    assert_eq!(
        outcome.error.as_ref().map(|e| e.kind()),
        Some("auth"),
        "error: {:?}",
        outcome.error
    );

    let reread = repo.get_by_id(tenant_id, config_id).await.expect("reread");
    assert!(
        !reread.verified,
        "a refused dial must never mark the config verified"
    );
}
