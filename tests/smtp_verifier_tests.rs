//! La verification SMTP doit DIALOGUER, pas repondre oui.
//!
//! Critere AC-006 (brief GTM, « Open Items » BA-001) : `POST /mail-configs/{id}/verify`
//! doit ouvrir une connexion TCP vers le serveur du tenant, saluer, s'authentifier, et
//! rendre le verdict du serveur. Tant que le seul temoin est un `verified = true` ecrit
//! en dur, un serveur injoignable ou des identifiants faux passent en silence.
//!
//! Ici on conduit l'implementation reelle (`LettreSmtpVerifier`) contre un faux serveur
//! SMTP local : aucune base de donnees, aucun reseau sortant.

mod common;

use std::time::Duration;

use base64::Engine as _;
use tokio::net::TcpListener;

use common::{AuthOutcome, FakeSmtpServer};
use ods_securemail::service::smtp_verifier::{
    LettreSmtpVerifier, SmtpTarget, SmtpVerifier, SmtpVerifyError,
};

fn verifier() -> LettreSmtpVerifier {
    LettreSmtpVerifier::new(Duration::from_secs(5))
}

#[tokio::test]
async fn a_reachable_server_that_accepts_the_credentials_verifies() {
    let server = FakeSmtpServer::start(AuthOutcome::Accept).await;
    let target = server.target("postmaster@example.com", "s3cret");

    verifier()
        .verify(&target)
        .await
        .expect("a server that greets and accepts AUTH must verify");

    let commands = server.commands();
    assert!(
        commands
            .iter()
            .any(|c| c.to_ascii_uppercase().starts_with("EHLO")),
        "the dial must greet the server (EHLO), transcript: {commands:?}"
    );
    let auth = commands
        .iter()
        .find(|c| c.to_ascii_uppercase().starts_with("AUTH"))
        .expect("the dial must attempt AUTH, not just open a socket");

    // Le secret voyage vraiment : AUTH PLAIN porte \0utilisateur\0mot-de-passe.
    let payload = auth
        .split_whitespace()
        .nth(2)
        .expect("AUTH PLAIN <payload>");
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(payload)
        .expect("AUTH PLAIN payload must be base64");
    let decoded = String::from_utf8(decoded).expect("utf-8");
    assert!(
        decoded.contains("postmaster@example.com") && decoded.contains("s3cret"),
        "the decrypted credentials must be the ones presented to the server"
    );
}

#[tokio::test]
async fn a_server_that_rejects_the_credentials_does_not_verify() {
    let server = FakeSmtpServer::start(AuthOutcome::Reject).await;
    let target = server.target("postmaster@example.com", "wrong-password");

    let err = verifier()
        .verify(&target)
        .await
        .expect_err("credentials refused by the server must not verify");

    assert_eq!(err.kind(), "auth", "got: {err:?}");
    assert!(
        !format!("{err:?}").contains("wrong-password"),
        "the error must never carry the password"
    );
}

#[tokio::test]
async fn a_closed_port_does_not_verify() {
    // Un port ephemere qu'on relache aussitot : rien n'y ecoute.
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let port = listener.local_addr().unwrap().port();
    drop(listener);

    let target = SmtpTarget {
        host: "127.0.0.1".to_string(),
        port,
        username: "postmaster@example.com".to_string(),
        password: "s3cret".to_string(),
        encryption: "none".to_string(),
    };

    let err = verifier()
        .verify(&target)
        .await
        .expect_err("an unreachable server must not verify");
    assert_eq!(err.kind(), "connect", "got: {err:?}");
}

#[tokio::test]
async fn a_server_that_never_answers_gives_up_on_the_timeout() {
    let server = FakeSmtpServer::start_mute().await;
    let target = server.target("postmaster@example.com", "s3cret");

    let verifier = LettreSmtpVerifier::new(Duration::from_millis(300));
    let started = std::time::Instant::now();
    let err = verifier
        .verify(&target)
        .await
        .expect_err("a mute server must not verify");

    assert_eq!(err.kind(), "connect", "got: {err:?}");
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "the dial must give up on its own timeout, took {:?}",
        started.elapsed()
    );
}

#[tokio::test]
async fn an_unknown_encryption_mode_is_a_configuration_error_not_a_dial() {
    let server = FakeSmtpServer::start(AuthOutcome::Accept).await;
    let mut target = server.target("postmaster@example.com", "s3cret");
    target.encryption = "ssl".to_string();

    let err = verifier()
        .verify(&target)
        .await
        .expect_err("an unsupported encryption mode must be refused");

    assert!(matches!(err, SmtpVerifyError::Config(_)), "got: {err:?}");
    assert!(
        server.commands().is_empty(),
        "nothing should have been dialled"
    );
}
