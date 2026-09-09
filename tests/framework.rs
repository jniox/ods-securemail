//! Garde de cadriciel : `h2` 0.3 ne doit pas revenir dans ce qu'on livre.
//!
//! RUSTSEC-2026-0258 (trames DATA vides non bornees, deni de service) n'a
//! aucun correctif dans la ligne 0.3. Le seul chemin qui l'amenait ici est la
//! fonctionnalite `http2` d'`actix-web`, activee par son jeu PAR DEFAUT ; le
//! manifeste prend donc les defauts moins `http2`.
//!
//! Voir `~/dev/projects/docstore/docs/security/h2-0.3-removal-runbook.md`
//! et la decision HR-20260909-001.
//!
//! Ces deux tests n'ont ni dependance ni base de donnees.

/// La preuve qui compte : `h2` 0.3 n'est pas dans ce qu'on livre.
///
/// Posée à cargo plutôt qu'au manifeste, pour tenir quel que soit le chemin
/// d'entrée : une fonctionnalité à nous, un satellite qui cesse de dire
/// `default-features = false`, une dépendance nouvelle.
#[test]
#[ignore = "blocked upstream: ods-common (branch staging, rev 4c538a07) declares \
            `actix-web = { version = \"4\", optional = true }` without \
            `default-features = false`, so IT activates actix-web/http2 and cargo \
            feature-unification keeps h2 0.3.27 in the lockfile no matter what this \
            manifest says. Remove this #[ignore] as soon as the ods-common fix \
            (work unit ods-common-c20260909-1345) is on staging and this repo's \
            Cargo.lock points at a rev that carries it. Verify with: \
            cargo tree -e normal -i h2@0.3.27"]
fn no_h2_zero_three_in_the_shipped_dependency_graph() {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let out = std::process::Command::new(cargo)
        .args(["tree", "--offline", "-e", "normal", "-i", "h2"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("cargo must be runnable to check the dependency graph");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);

    // `h2` 0.4 (hyper, reqwest, tonic) est la ligne corrigee : elle a le droit
    // d'etre la. L'absence totale du paquet est un resultat valide aussi.
    assert!(
        stderr.contains("did not match any packages") || !stdout.trim().is_empty(),
        "could not read the dependency graph, so nothing was checked:\n{stderr}"
    );
    let offenders: Vec<&str> = stdout.lines().filter(|l| l.contains("h2 v0.3")).collect();
    assert!(
        offenders.is_empty(),
        "h2 0.3 is back in the shipped graph (RUSTSEC-2026-0258). Reached by:\n{}",
        offenders.join("\n")
    );
}

/// Le versant manifeste du meme fait, pour que l'echec soit lisible.
#[test]
fn the_manifest_does_not_ask_for_http2_by_any_of_its_names() {
    let manifest = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml"),
    )
    .expect("Cargo.toml must be readable");
    // L'entree, accolades equilibrees : s'arreter a la ligne suivante marcherait
    // pour une entree d'une ligne et lirait tout le reste du manifeste pour une
    // entree multiligne — et « rustls » finirait par matcher une feature de sqlx.
    let mut lines = manifest
        .lines()
        .skip_while(|l| !l.trim_start().starts_with("actix-web"));
    let first = lines.next().expect("[dependencies] must declare actix-web");
    let depth = |s: &str| s.matches('{').count() as i32 - s.matches('}').count() as i32;
    let mut entry = String::from(first);
    let mut open = depth(first);
    for line in lines {
        if open <= 0 {
            break;
        }
        open += depth(line);
        entry.push('\n');
        entry.push_str(line);
    }

    assert!(
        entry.contains("default-features = false"),
        "actix-web's default feature set turns on `http2`, the only thing pulling \
         h2 0.3 into this binary:\n{entry}"
    );
    for dangerous in ["\"http2\"", "rustls", "\"openssl\""] {
        assert!(
            !entry.contains(dangerous),
            "the actix-web feature {dangerous} pulls h2 0.3 back in:\n{entry}"
        );
    }
}
