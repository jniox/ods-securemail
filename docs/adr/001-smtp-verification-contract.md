# ADR-001 — La verification d'une configuration SMTP compose vraiment, et rend un verdict

- **Statut** : accepte
- **Date** : 2026-09-11
- **Portee** : securemail, point d'entree `POST /api/v1/mail-configs/{id}/verify`
- **Contexte amont** : revue BA du 2026-09-10 (AC-006 MISSING, deviation HIGH) ;
  brief GTM `pdlc/gtm/securemail-gtm.md`, « Open Items » BA-001.

## Contexte

Le point d'entree rendait `{"verified": true, "message": "Connection successful"}` sans
jamais ouvrir de connexion, et ecrivait `verified = true` en base. Consequence : une
configuration pointant vers un hote injoignable, ou portant un mot de passe faux, passait
la verification en silence — exactement ce qu'un tenant vient chercher ici.

Trois questions se posaient, dont deux seulement sont techniques.

## Decision

### 1. La composition est reelle, et elle va jusqu'a AUTH

`LettreSmtpVerifier` (sur `lettre`) ouvre la connexion TCP, applique la politique TLS
deduite de `smtp_encryption` (`tls` → TLS implicite ; `starttls` → bascule EXIGEE, jamais
opportuniste ; `none` → clair), envoie EHLO, puis presente les identifiants dechiffres.
Une poignee de main qui reussit mais une authentification qui echoue n'est PAS une
verification : le critere d'acceptation dit « TCP handshake + AUTH », et c'est l'AUTH qui
distingue « ce serveur existe » de « ce serveur nous accepte ».

### 2. Le point d'entree rend 200 avec un verdict, pas un code d'erreur HTTP

`{"config_id", "verified", "message", "reason"?, "checked_at"}`, avec `reason` dans
`connect | auth | protocol | config`.

Motif : c'est un DIAGNOSTIC. « Le serveur du tenant refuse ces identifiants » est un
resultat de la verification, pas une panne de securemail — le rendre en 4xx/5xx melangerait
« la verification a echoue » et « la verification n'a pas pu avoir lieu ». Les deux cas ou
la verification ne peut PAS etre conduite gardent bien leur code d'erreur : 404 si la
configuration n'existe pas pour ce tenant, 500 si notre propre cle de chiffrement ne
rouvre plus le coffre. Accessoirement, `AppError` (ods-common) n'a aucune variante qui
rende 502, et en inventer une pour ce seul usage aurait etendu un type partage par toute
la plateforme pour un besoin local.

### 3. Le drapeau `verified` suit le DERNIER appel, dans les deux sens

Un echec repasse la configuration a `verified = false`. Sans cela, le drapeau raconterait
le passe : verifie une fois en mars, toujours « verifie » en septembre alors que le mot de
passe a expire. L'evenement CloudEvents `ods.securemail.config.verified` n'est emis que sur
succes — le contrat d'evenement du brief GTM n'en declare pas d'autre.

### 4. Toute composition est bornee dans le temps

`SMTP_VERIFY_TIMEOUT_SECS` (defaut 10 s, borne a [1, 60]) encadre l'echange ENTIER.
Piege mesure le 2026-09-11 : le delai que `lettre` accepte ne couvre que l'etablissement
TCP. Un serveur qui accepte la connexion puis se tait laisse la requete pendue
indefiniment. La borne est donc posee par nous autour de l'appel complet, et un test
(`a_server_that_never_answers_gives_up_on_the_timeout`) tient ce fait.

Cette borne est aussi une mesure de securite : `/verify` est, par construction, une porte
qui fait composer un hote choisi par l'appelant (SSRF). Le protocole parle SMTP, la reponse
rendue ne contient jamais le texte du serveur distant, et l'appel ne peut pas durer.

## Consequences

- `MailConfigService` recoit un `Arc<dyn SmtpVerifier>` : les tests de route injectent
  `FixedSmtpVerifier`, la production `LettreSmtpVerifier`. Le jour ou IMAP doit etre
  verifie, il prend le meme chemin.
- Le depot expose `get_with_smtp_password`, distinct de `get_by_id` : le secret ne sort
  que lorsqu'on en a besoin pour composer, et l'appelant doit le demander.
- Nouvelle dependance `lettre`, sans `dkim` (qui tirerait `rsa` 0.9 / RUSTSEC-2023-0071)
  ni `native-tls` (qui tirerait OpenSSL). Graphe livre mesure apres coup : `h2` 0.3 et
  `protobuf` 2.x toujours absents, `cargo audit` inchange.
- Reste ouvert, hors de ce lot : la liste des ports admis (25/465/587) exclut le 2525,
  couramment propose par les relais ; et `imap_*` n'est pas verifie du tout.
