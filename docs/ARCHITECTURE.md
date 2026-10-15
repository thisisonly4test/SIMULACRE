# Architecture

## Vue d'ensemble

```
navigateur ──HTTP/JSON──▶ simulacre-server (axum)
                            ├─ simulacre-core   génération pure, déterministe
                            ├─ SQLite        coffre : personas, journal, visages
                            ├─ faces/        JPEG ré-encodés (512², sans métadonnées)
                            └─ reqwest ────▶ thispersondoesnotexist.com (seul appel sortant)
```

## simulacre-core

Le moteur n'a aucune E/S. Ses données sont embarquées avec `include_str!` et lues une seule fois (`OnceLock`).

### Recette → persona

```rust
Recipe { seed, locale, archetype?, gender?, age_min?, age_max?, as_of, rerolls: {section → n} }
```

- `as_of` fige la date de référence. Une légende rouverte un an plus tard garde donc le même âge et la même chronologie.
- Chaque section tire ses valeurs d'un flux ChaCha8 dont la graine est `sha256(seed ‖ section ‖ rerolls[section])` (voir `rng.rs`). Retirer une section n'avance que son propre flux.

### Ordre de génération (`generate/mod.rs`)

| Étape | Dépend de | Produit |
|---|---|---|
| archétype | graine, tranche d'âge | profil de vie |
| identity | archétype | genre, naissance, prénom selon l'année, lieu, langues |
| interests | âge, archétype | 3 à 5 loisirs, avec une dérive selon l'âge |
| life | identité | statut, études, carrière, ville, logement |
| family | identité, loisirs | partenaire, enfants, fratrie, parents, animaux |
| personality | âge, archétype | Big Five avec effet de maturité, profil, traits |
| appearance | identité, loisirs | taille selon le pays, corpulence, cheveux selon l'âge |
| behavior | personnalité, emploi du temps | chronotype, carte 7×24, appareils, style d'écriture |
| digital | tout ce qui précède | plateformes, pseudos, bios, années d'inscription |
| voice | style, carte, comptes | publications stylisées et horodatées |
| story | persona | chronologie, récit FR/EN, légende (pitch, points d'appui, points faibles, opsec) |
| coherence | persona | score, contrôles, exposition |

`refresh()` relance `story` et `coherence` après une modification manuelle. Les sections générées restent telles quelles, car ce sont les modifications de l'opérateur.

### Données

- `data/common.json` contient 12 secteurs et leurs métiers (genrés en FR et DE), 32 loisirs (libellés et anecdotes en FR, EN, DE), 12 archétypes, 23 plateformes avec leurs règles de pseudo et leur courbe d'âge, et les mots des noms de code.
- `data/locales/*.json` contient les prénoms avec leur année de pic de popularité, les noms, 14 à 15 villes (quartiers, équipe, monument, spécialité, gentilé, université, météo), le système scolaire, les modèles d'employeurs et le plan téléphonique.
- `data/lang/*.json` contient les modèles de bios par plateforme, de publications par sujet, et l'argot par tranche d'âge.

Pour ajouter un pays : écrire un fichier `locales/xx_XX.json`, puis l'enregistrer dans `data.rs`. Le test `data_references_resolve` signale tout champ manquant.

## simulacre-server

- **axum 0.8**. Les routes sont dans `lib.rs` et les handlers dans `api.rs`.
- **Store** : SQLite (rusqlite, `bundled`) en WAL derrière un `Mutex`. Chaque appel est une requête courte. Le journal est supprimé en cascade avec sa légende.
- **Visages** (`faces.rs`) : téléchargement, décodage, recadrage carré, redimensionnement à 512 px (Lanczos3), puis JPEG qualité 88. L'identifiant est le `sha256` des octets produits (32 hex). Cette chaîne a trois effets :
  1. aucune métadonnée n'est conservée ;
  2. le même visage utilisé deux fois est détecté (contamination, badge « déjà utilisé ») ;
  3. l'identifiant validé par `is_valid_id` sert aussi de nom de fichier, ce qui empêche la traversée de chemin.
  
  La source renvoie souvent la même image à des requêtes simultanées. Le serveur télécharge donc par vagues espacées et relance les doublons, avec un nombre de tentatives borné.
- **Sécurité** : écoute sur la boucle locale par défaut. Le serveur refuse d'écouter ailleurs sans `SIMULACRE_TOKEN`. La comparaison du jeton se fait en temps constant. Le jeton est accepté en en-tête `Bearer` ou en cookie, ce dernier servant aux balises `<img>`. Les en-têtes CSP, `nosniff`, `no-referrer` et `X-Frame-Options: DENY` sont envoyés, et la taille des corps de requête est limitée.

## Version sans serveur (`simulacre-wasm`, `web/js/local.js`, `worker/`)

Les routes HTTP et la version WebAssembly partagent leur logique dans `simulacre_core::service` : recette reçue, vivier, catalogue, règles du journal, note d'édition, export avec journal, validation des identifiants de visage. Les deux répondent donc de la même façon.

- `crates/simulacre-wasm` expose ces fonctions à JavaScript (`wasm-bindgen`, échanges en JSON). « Aujourd'hui » est passé par l'appelant, et l'aléa des graines vient de `crypto.getRandomValues` (`getrandom`, fonctionnalité `js`). La sortie de `scripts/build-wasm.sh` est versionnée dans `web/pkg/` : l'hébergement statique n'a pas besoin de Rust.
- `web/js/api.js` choisit le moteur au premier appel : le serveur Rust s'il répond à `/api/health` avec `backend: "server"`, sinon `local.js`, chargé à la demande avec le WebAssembly. Le serveur Rust ne charge donc jamais le WebAssembly.
- `local.js` reproduit l'API sur IndexedDB (magasins `personas`, `journal`, `faces`), avec les mêmes codes et messages d'erreur. Chaque écriture qui touche deux magasins (légende et journal) se fait dans une seule transaction. Les visages sont ré-encodés dans le navigateur comme sur le serveur : recadrage carré, 512 px, JPEG 0,88, identifiant égal aux 128 premiers bits du SHA-256. Ils sont affichés par des URL `blob:`.
- `worker/index.js` sert `web/` et ne fait qu'une chose en plus : relayer `GET /api/face` depuis l'URL fixe de thispersondoesnotexist.com. Les autres routes `/api/*` répondent 404. `web/_headers` applique la même CSP que le serveur, plus `'wasm-unsafe-eval'`.
- Les tests Playwright tournent sur les deux versions. `tests/e2e/static-server.mjs` imite le Worker.

## Interface (`web/`)

Elle est écrite en modules ES, sans étape de build. Le DOM est construit avec `h()`, et toute donnée passe par `textContent` : aucun `innerHTML` ne reçoit de données. Chaque navigation rend sa vue dans un conteneur neuf (`.view`, en `display: contents`) : une vue encore en chargement quand on change de page écrit dans un nœud détaché, jamais par-dessus la page suivante. Le routage se fait par hash : `#/`, `#/pool`, `#/vault`, `#/p/:id`, `#/contamination`, `#/doctrine`. L'interface est en français et en anglais. Les libellés des énumérations viennent de `/api/meta`, qui est la source unique `labels.rs`.

Identité visuelle « imprimerie de sécurité » (`security.js`, `style.css`) :

- le fond porte un motif ondulé de billet de banque généré en SVG au chargement ;
- la page de passeport SPECIMEN porte une MRZ calculée selon ICAO 9303 (`mrz()`, chiffres de contrôle 7-3-1, émetteur `UTO`) ;
- l'hologramme suit le pointeur via `--hx` et `--hy` ;
- **marionnette** (`puppet.js`) : le logo est une poupée de fille en silhouette (grosse tête ronde, couettes, nœud, robe trapèze), une ombre sans visage. Sur l'accueil, les fils descendent du haut de la page : le contrôleur reste invisible au-dessus, et le point de départ des fils est recalculé quand la mise en page change (`ResizeObserver`). Une petite simulation anime la poupée. Le contrôleur oscille, chaque fil tire son point d'attache (mains, genoux, tête) via un ressort amorti, les os sont des contraintes de longueur fixe, les coudes sont résolus par cinématique inverse à deux segments, les couettes se balancent comme des pendules et la robe suit le corps avec un léger retard. La `viewBox` est zoomée sur la poupée, et le calcul du haut de page en tient compte. Le dessin est en deux calques : les fils restent sous la barre du haut et sous le calque UV, et la poupée passe au-dessus du calque UV. La poupée est une ombre : très transparente (opacité 0,3) et floutée. Les fils sont masqués par sa silhouette (un `<mask>` qui réutilise le corps via `<use>`, passé en noir opaque par un `feColorMatrix`), ce qui évite de les voir à travers elle. Le pointeur déplace le contrôleur. L'animation s'arrête hors écran (`IntersectionObserver`) et quand l'élément quitte le DOM. Elle reste figée si `prefers-reduced-motion` est actif. Le logo de la barre et `assets/favicon.svg` sont la même figure en version statique ;
- **lampe UV** : un calque fixe de fibres fluorescentes et de micro-texte, affiché seulement sous `data-theme="uv"`, qui bascule aussi toute la palette en encres fluorescentes. Le choix est mémorisé, la touche `U` bascule le mode. Le papier est toujours le thème par défaut : l'UV ne s'active que sur demande.
- **curseur** : une loupe en SVG inline (`--cur`, et `--cur-hot` à verre teinté sur les éléments cliquables), redéfinie pour le thème UV. Les champs de saisie gardent le curseur texte.

La seule image livrée est le favicon SVG, et aucune police web n'est chargée.
