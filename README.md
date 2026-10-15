# SIMULACRE — Toute couverture a besoin d'une histoire

> Forge d'identités fictives (« sock puppets ») pour l'OSINT, écrite en Rust.

> « Le simulacre n'est jamais ce qui cache la vérité — c'est la vérité qui cache qu'il n'y en a pas. Le simulacre est vrai. »
> — L'Ecclésiaste, selon Jean Baudrillard, *Simulacres et simulation* (1981)

Les générateurs habituels donnent un nom, une ville et une date de naissance. Ça ne tient pas trois messages face à quelqu'un d'attentif. SIMULACRE forge une **légende** : une personne fictive dont chaque détail tient avec les autres, et qu'un enquêteur peut apprendre, tenir dans la durée et suivre.

| | Générateur classique | SIMULACRE |
|---|---|---|
| Identité | nom, âge, ville | prénoms choisis selon l'année de naissance, lieu de naissance, langues (dont les langues d'origine suggérées par le nom), signe, apparence |
| Parcours | — | études → emplois → déménagements → comptes, datés, ordonnés, et réalistes selon le pays (Bac/BTS, A-levels, Abitur…) |
| Entourage | — | partenaire, enfants, fratrie, parents (vivants ou non), animaux, tous avec des âges cohérents |
| Psychologie | — | Big Five, profil, valeurs, manies, agacements, humour |
| Rythme | — | carte 7×24 des heures en ligne, déduite du chronotype, de l'emploi du temps et de l'extraversion |
| Réseaux | un pseudo | plateformes choisies selon l'âge et les loisirs, pseudos **validés selon les règles de chaque site**, bios, années d'inscription plausibles |
| Voix | — | exemples de publications dans la langue de la légende, avec son registre, ses emojis, son argot, ses tics et ses fautes, placés aux heures où elle est en ligne |
| Visage | — | planche contact de visages StyleGAN ([thispersondoesnotexist.com](https://thispersondoesnotexist.com)), ré-encodés en local |
| Vérification | — | **moteur de cohérence** : plus de 30 contrôles, relancés à chaque modification, avec un score |
| Durée | — | coffre, cycle de vie (brouillon → incubation → active → dormante → grillée), journal d'opérations |
| Opsec | — | **analyse de contamination** entre légendes, score d'exposition des pseudos, interrogatoire chronométré |

## Ce qui est nouveau

- **Déterministe.** Une légende est une fonction pure de sa recette. `SIM-7Q4K-M2XD` redonne toujours la même personne. Il y a 1,1 × 10¹² graines possibles, et le **Vivier** les parcourt sans fin.
- **Historique de forge.** Appuyer à nouveau sur « Forger » sans toucher la graine produit une nouvelle personne. Une graine tapée à la main, tirée au dé ou venue d'un lien est respectée. Les 30 dernières légendes sont gardées pour la session, avec leurs modifications, retirages et visages. On circule entre elles avec Précédente / Suivante ou en cliquant leur nom. Forger après être revenu en arrière efface les légendes qui suivaient, comme l'historique d'un navigateur.
- **Retirage par section.** Chaque section a son propre flux aléatoire. On peut retirer la voix sans perdre le parcours.
- **Moteur de cohérence.** On modifie la date de naissance, un pseudo ou un poste, puis le moteur revérifie tout et explique ce qui casse. Exemples : « @Not Valid!! invalide sur X (caractère ' ' interdit). Suggestion : @Not_Valid », « Compte Instagram ouvert à 11 ans ».
- **Contamination.** Deux légendes du même opérateur ne doivent rien partager : visage (identifiant par hash du contenu), pseudo (plus longue sous-chaîne commune), numéro, quartier, employeur, bio, et même **les heures de connexion** (corrélation de Pearson des cartes 7×24).
- **Exposition.** Les vraies personnes réutilisent leurs pseudos. SIMULACRE reproduit cette habitude, puis mesure à quel point un pseudo mène aux autres comptes.
- **Interrogatoire.** Dix questions qu'un contact curieux poserait, avec des réponses fausses plausibles tirées du même pays. Le temps de réponse moyen est mesuré, parce que l'hésitation se voit.
- **Points faibles.** La légende liste ce qui peut la trahir : « X affiche la date de création alors que la légende dit "depuis 2014" », « La légende parle allemand couramment : évite les échanges dans cette langue si ce n'est pas ton cas ».

## Identité visuelle

L'interface reprend les codes de l'impression de papiers d'identité : papier crème, motif de billet généré en SVG, encres taille-douce, et chaque légende rendue comme une **page de passeport SPECIMEN**. Cette page comporte un hologramme qui réagit au curseur, une image fantôme, une signature et une **bande MRZ conforme ICAO 9303** avec ses chiffres de contrôle. L'émetteur est `UTO`, le code qu'ICAO réserve aux spécimens. Les statuts s'affichent comme des tampons encreurs. Le logo est une **marionnette** : une poupée de fille en silhouette, comme une ombre, avec couettes, nœud et robe, sans visage. Sur l'accueil, ses fils descendent du haut de la page, la tirent et la font bouger. Le curseur déplace la main invisible qui la tient.

**Lampe UV** (désactivée par défaut, quel que soit le thème du système) : le curseur révèle l'encre invisible du papier, avec ses fibres fluorescentes et son micro-texte « FICTIF · SPECIMEN ». La touche `U` ou le bouton UV passe toute la page sous lumière noire.

## Garde-fous (non négociables)

- **18 ans minimum**, imposé par le moteur (`InvalidAgeRange`).
- **Pas d'adresse postale** : seulement la ville et le quartier.
- **Pas** de numéro de sécurité sociale, de carte d'identité, d'IBAN ni de carte bancaire.
- **Téléphones** uniquement dans des plages réservées à la fiction : ARCEP (décision n° 2018-0881 : `06 39 98`), Ofcom (`07700 900xxx`), NANP (`555-01xx`). Pour l'Allemagne, qui n'a pas de plage officielle : aucun numéro.
- **Aucune automatisation des comptes** : pas de création de compte, pas de contournement de captcha. La plateforme prépare, un humain agit et en répond.
- Visages générés uniquement. Les métadonnées sont supprimées au ré-encodage.

Voir [docs/DOCTRINE.md](docs/DOCTRINE.md).

## Lancer

Prérequis : Rust ≥ 1.85 (édition 2024).

```bash
cargo run --release -p simulacre-server
# → http://127.0.0.1:7878   (coffre et visages dans ./simulacre-data)
```

| Variable / option | Défaut | Rôle |
|---|---|---|
| `SIMULACRE_ADDR` / `--addr` | `127.0.0.1:7878` | adresse d'écoute |
| `SIMULACRE_DATA` / `--data` | `./simulacre-data` | coffre SQLite + visages |
| `SIMULACRE_TOKEN` / `--token` | — | jeton exigé sur `/api` (**obligatoire hors localhost** : le serveur refuse de démarrer sans) |
| `SIMULACRE_FACE_URL` | thispersondoesnotexist.com | source des visages |

La plateforme s'appelait auparavant LEGEND. Au premier lancement, un ancien dossier `./legend-data` est renommé en `./simulacre-data` si celui-ci n'existe pas encore. Les réglages du navigateur (langue, lampe UV, sections repliées, jeton, historique de forge) sont repris automatiquement. Les anciennes graines `LGD-…` restent valides : une graine est une chaîne libre, seules les nouvelles commencent par `SIM-`.

Le serveur est un binaire unique : l'interface est embarquée dedans. Il n'y a pas de dépendance JavaScript à l'exécution. Le navigateur ne contacte jamais de tiers : pas de polices web, pas de CDN, CSP `default-src 'self'`.

## Version en ligne (Cloudflare, sans serveur)

Le même site fonctionne sans le serveur Rust, par exemple sur Cloudflare Workers :

- le moteur `simulacre-core` est compilé en WebAssembly (`crates/simulacre-wasm`, sortie dans `web/pkg/`, versionnée) et tourne dans le navigateur ;
- le coffre, le journal et les visages sont stockés dans le navigateur (IndexedDB). Rien n'est envoyé en ligne, mais chaque visiteur a son propre coffre, et effacer les données du site le vide : exporte en JSON pour garder une copie ;
- un petit Worker (`worker/index.js`) sert les fichiers de `web/` et relaie uniquement les visages de thispersondoesnotexist.com, que le navigateur ne peut pas récupérer seul (pas d'en-têtes CORS). Il ne stocke rien.

L'interface choisit d'elle-même : si `/api/health` répond `backend: "server"`, elle utilise le serveur Rust ; sinon, le moteur du navigateur.

Déploiement : `wrangler.jsonc` est à la racine. Dans Cloudflare (Workers & Pages → le Worker → Settings → Build), la commande de déploiement doit être `npx wrangler deploy`, sans commande de build ni dossier de sortie. Pour tester en local : `npx wrangler dev`.

Après une modification du moteur Rust, régénérer `web/pkg/` :

```bash
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.129   # même version que la crate wasm-bindgen
./scripts/build-wasm.sh
```

Pour une légende en ligne de commande :

```bash
cargo run -p simulacre-core --example forge -- SIM-DEMO-0001 fr_FR parent fr
```

## Tests

```bash
cargo test                     # 63 tests : moteur (600 graines × 4 pays) + API HTTP
cargo clippy --all-targets     # 0 avertissement
npm install && npx playwright test   # 40 tests navigateur : serveur Rust et version sans serveur, bureau + mobile, hors ligne
```

Ce que couvrent les tests :

- **Moteur** : déterminisme, indépendance des retirages, aucune erreur de cohérence sur 600 légendes, pseudos conformes aux règles des plateformes, téléphones dans les plages fictives, aucun gabarit non rempli, libellés présents pour chaque clé, intégrité des données.
- **API** : cycle de vie complet, jeton (en-tête et cookie), en-têtes de sécurité, traversée de chemin sur `/faces`, visages dédupliqués, source de visages hors ligne.
- **E2E** : forge, langue, édition et recontrôle, retirage, visage, coffre, journal, contamination, vivier infini, interrogatoire, absence de débordement sur mobile, aucune erreur console ni violation CSP.

Si Playwright ne trouve pas son Chromium : `CHROMIUM_PATH=/chemin/vers/chrome npx playwright test`.

## Structure

```
crates/simulacre-core/    moteur pur (aucune E/S) : données, génération, cohérence, contamination, quiz, export
  data/                pays (fr_FR, en_US, en_GB, de_DE), langues (fr, en, de), catalogue commun
crates/simulacre-server/  axum + SQLite + forge de visages + UI embarquée
crates/simulacre-wasm/    le moteur pour le navigateur (version sans serveur)
web/                   interface (HTML/CSS/JS sans build, identité « imprimerie de sécurité ») ; web/pkg/ = moteur WebAssembly
worker/                Worker Cloudflare : fichiers statiques + relais des visages
scripts/               build-wasm.sh
tests/e2e/             Playwright (serveur Rust et version sans serveur) + sources de visages factices
docs/                  architecture, cohérence, API, doctrine
```

Détails : [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) · [docs/COHERENCE.md](docs/COHERENCE.md) · [docs/API.md](docs/API.md).
