//! Garde de cadriciel : ce qu'on livre ne doit pas ramener d'analyseur vulnerable.
//!
//! RUSTSEC-2026-0258 (trames DATA vides non bornees, deni de service) n'a
//! aucun correctif dans la ligne 0.3 de `h2`. Le seul chemin qui l'amenait ici est
//! la fonctionnalite `http2` d'`actix-web`, activee par son jeu PAR DEFAUT ; le
//! manifeste prend donc les defauts moins `http2`.
//!
//! Voir `~/dev/projects/docstore/docs/security/h2-0.3-removal-runbook.md`,
//! la decision HR-20260909-001 et sa suite HR-20260909-031.
//!
//! Ces tests n'ont ni dependance ni base de donnees : ils interrogent cargo.

/// Le graphe tel que cargo le resout, en arbre AVANT.
///
/// Volontairement pas `-i <crate>` (arbre inverse) : celui-la echoue avec
/// « ambiguous » des que deux versions majeures coexistent, c'est-a-dire
/// exactement dans le cas qu'on veut voir nomme. L'arbre avant, lui, repond
/// toujours, et la ligne fautive porte son chemin.
fn shipped_dependency_tree(edges: &[&str]) -> String {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let out = std::process::Command::new(cargo)
        .args(["tree", "--offline", "-e", &edges.join(",")])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("cargo must be runnable to check the dependency graph");
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(
        out.status.success() && !stdout.trim().is_empty(),
        "could not read the dependency graph, so nothing was checked:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    stdout
}

/// La preuve qui compte : `h2` 0.3 n'est pas dans ce qu'on livre.
///
/// Posee a cargo plutot qu'au manifeste, pour tenir quel que soit le chemin
/// d'entree : une fonctionnalite a nous, un satellite qui cesse de dire
/// `default-features = false`, une dependance nouvelle.
///
/// `h2` 0.4 (hyper, reqwest) est la ligne corrigee des 0.4.16 : elle a le droit
/// d'etre la. On regarde aussi les aretes de developpement, parce que le
/// lockfile — ce que lit `cargo audit` — ne les distingue pas.
#[test]
fn no_h2_zero_three_in_the_shipped_dependency_graph() {
    let stdout = shipped_dependency_tree(&["normal", "dev"]);
    let offenders: Vec<&str> = stdout.lines().filter(|l| l.contains("h2 v0.3")).collect();
    assert!(
        offenders.is_empty(),
        "h2 0.3 is back in the shipped graph (RUSTSEC-2026-0258, no fix in the 0.3 line). \
         Reached by:\n{}",
        offenders.join("\n")
    );
}

/// La contrepartie : ne pas troquer un avis contre un autre.
///
/// `ods-common` expose une fonctionnalite `metrics` (endpoint Prometheus) entree
/// dans son jeu `full` en meme temps que le correctif h2. Or ce service ne sert
/// aucun `/metrics` : demander `full` faisait entrer `prometheus` 0.13 et donc
/// `protobuf` 2.28.0 — RUSTSEC-2024-0437, recursion non bornee a l'analyse — dans
/// le graphe LIVRE, au moment precis ou l'on en retirait `h2` 0.3.
///
/// Le manifeste nomme donc les fonctionnalites reellement consommees. Cette garde
/// tient ce choix, en interrogeant cargo et non le manifeste.
#[test]
fn no_protobuf_parser_in_the_shipped_dependency_graph() {
    let stdout = shipped_dependency_tree(&["normal"]);
    let offenders: Vec<&str> = stdout
        .lines()
        .filter(|l| l.contains("protobuf v2."))
        .collect();
    assert!(
        offenders.is_empty(),
        "protobuf 2.x is in the shipped graph (RUSTSEC-2024-0437). This service parses no \
         protobuf; it most likely arrived through an ods-common feature set it does not use \
         (`full` pulls in `metrics` -> prometheus -> protobuf). Reached by:\n{}",
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
