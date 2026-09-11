//! Faux serveur SMTP partage par les tests d'integration.
//!
//! Il parle assez de SMTP pour qu'un vrai client (`lettre`) aille au bout d'une poignee
//! de main et d'une authentification, et il GARDE LA TRANSCRIPTION des commandes recues :
//! c'est elle qui prouve qu'il y a eu dialogue, et que le mot de passe presente est bien
//! celui qui avait ete chiffre.

#![allow(dead_code)]

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};

use ods_securemail::service::smtp_verifier::SmtpTarget;

/// Ce que le faux serveur repond a `AUTH`.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum AuthOutcome {
    Accept,
    Reject,
}

/// Un serveur SMTP minimal, en clair, sur un port ephemere.
pub struct FakeSmtpServer {
    pub addr: SocketAddr,
    transcript: Arc<Mutex<Vec<String>>>,
}

impl FakeSmtpServer {
    /// Parle SMTP et accepte (ou refuse) l'authentification.
    pub async fn start(outcome: AuthOutcome) -> Self {
        Self::spawn(move |_| outcome, true).await
    }

    /// Accepte la connexion TCP et ne dit plus jamais rien : le cas du serveur
    /// qui ecoute mais ne repond pas, que seul un delai d'attente peut trancher.
    pub async fn start_mute() -> Self {
        Self::spawn(|_| AuthOutcome::Accept, false).await
    }

    async fn spawn<F>(decide: F, speak: bool) -> Self
    where
        F: Fn(&str) -> AuthOutcome + Send + Sync + 'static,
    {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("local_addr");
        let transcript = Arc::new(Mutex::new(Vec::new()));
        let recorder = transcript.clone();

        tokio::spawn(async move {
            while let Ok((stream, _)) = listener.accept().await {
                let recorder = recorder.clone();
                let decide = &decide;
                if !speak {
                    // On garde la connexion ouverte, muette, jusqu'au delai du client.
                    tokio::time::sleep(Duration::from_secs(30)).await;
                    continue;
                }
                if let Err(e) = serve(stream, recorder, decide).await {
                    eprintln!("fake smtp server: {e}");
                }
            }
        });

        FakeSmtpServer { addr, transcript }
    }

    pub fn commands(&self) -> Vec<String> {
        self.transcript.lock().unwrap().clone()
    }

    pub fn target(&self, username: &str, password: &str) -> SmtpTarget {
        SmtpTarget {
            host: "127.0.0.1".to_string(),
            port: self.addr.port(),
            username: username.to_string(),
            password: password.to_string(),
            encryption: "none".to_string(),
        }
    }
}

async fn serve<F>(
    stream: TcpStream,
    transcript: Arc<Mutex<Vec<String>>>,
    decide: &F,
) -> std::io::Result<()>
where
    F: Fn(&str) -> AuthOutcome,
{
    let (read_half, mut write_half) = stream.into_split();
    let mut lines = BufReader::new(read_half).lines();

    write_half
        .write_all(b"220 fake.test ESMTP ready\r\n")
        .await?;

    while let Some(line) = lines.next_line().await? {
        transcript.lock().unwrap().push(line.clone());
        let upper = line.to_ascii_uppercase();

        let reply: String = if upper.starts_with("EHLO") {
            // Multiligne : seule la derniere porte une espace apres le code.
            "250-fake.test\r\n250-AUTH PLAIN\r\n250 OK\r\n".to_string()
        } else if upper.starts_with("HELO") {
            "250 fake.test\r\n".to_string()
        } else if upper.starts_with("AUTH") {
            let payload = line.split_whitespace().nth(2).unwrap_or("").to_string();
            match decide(&payload) {
                AuthOutcome::Accept => "235 2.7.0 Authentication succeeded\r\n".to_string(),
                AuthOutcome::Reject => {
                    "535 5.7.8 Authentication credentials invalid\r\n".to_string()
                }
            }
        } else if upper.starts_with("QUIT") {
            write_half.write_all(b"221 2.0.0 Bye\r\n").await?;
            return Ok(());
        } else {
            "250 2.0.0 Ok\r\n".to_string()
        };

        write_half.write_all(reply.as_bytes()).await?;
    }
    Ok(())
}
