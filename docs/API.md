# API HTTP

Toutes les routes échangent du JSON. Les erreurs sont renvoyées sous la forme `{ "error": "…" }`, avec le code 400, 401, 404, 502 ou 500.

Si `SIMULACRE_TOKEN` est défini, chaque route `/api/*` et `/faces/*` exige `Authorization: Bearer <jeton>` ou le cookie `simulacre_token`. Seule `/api/health` reste publique.

## Catalogue et forge

| Méthode | Route | Corps / requête | Réponse |
|---|---|---|---|
| GET | `/api/health` | — | `{ok:true, backend:"server"}` (le Worker Cloudflare répond `backend:"browser"`) |
| GET | `/api/meta` | — | pays, archétypes, plateformes, secteurs, loisirs, sections, statuts, libellés, limites |
| GET | `/api/seed` | — | `{seed}` |
| POST | `/api/forge` | `{seed?, locale?, archetype?, gender?, age_min?, age_max?, as_of?, rerolls?}` | `Persona` |
| GET | `/api/pool` | `?pool&offset&limit≤60&locale&archetype&gender` | `{items:[Summary+locale], next}` |
| POST | `/api/check` | `Persona` (modifiée) | `Persona` (chronologie, récit et cohérence recalculés) |
| POST | `/api/quiz` | `{persona, round}` | `[{prompt:{fr,en}, choices[], answer}]` |
| POST | `/api/export` | `{persona, lang}` | `text/markdown` |

## Coffre

| Méthode | Route | Corps | Réponse |
|---|---|---|---|
| GET | `/api/personas` | — | `[{id, status, created_at, updated_at, face, …Summary}]` |
| POST | `/api/personas` | `Persona` | 201 `Record` |
| GET | `/api/personas/{id}` | — | `Record` |
| PUT | `/api/personas/{id}` | `Persona` | `Record` (recontrôlé, entrée ajoutée au journal) |
| DELETE | `/api/personas/{id}` | — | 204 |
| PUT | `/api/personas/{id}/status` | `{status}` (`draft`, `incubating`, `active`, `dormant` ou `burned`) | `Record` |
| GET / POST | `/api/personas/{id}/journal` | `{kind: note, action, contact ou incident ; text ≤ 4000}` | entrées |
| GET | `/api/personas/{id}/quiz?round=n` | — | questions |
| GET | `/api/personas/{id}/export.md?lang=fr` | — | fiche avec journal |
| GET | `/api/personas/{id}/export.json` | — | `{record, journal}` |
| GET | `/api/vault/backup` | — | sauvegarde complète (voir plus bas), en pièce jointe `simulacre-vault-AAAA-MM-JJ.json` |
| POST | `/api/vault/restore` | une sauvegarde complète, ou un export `{record, journal}` (512 Mo maximum) | `{added, updated, skipped, faces, faces_missing}` |
| GET | `/api/stats` | — | `{total, by_status, avg_coherence}` |
| GET | `/api/contamination` | — | `{pairs:[{a,b,risk,links}], names}` |

### Sauvegarde du coffre

Format commun au serveur et à la version navigateur : une sauvegarde prise dans l'une se restaure dans l'autre.

```json
{ "format": "simulacre-vault", "version": 1, "exported_at": "2026-10-16T12:00:00Z",
  "records": [{ "id", "status", "created_at", "updated_at", "persona", "journal": [{ "at", "kind", "text" }] }],
  "faces": [{ "id", "source", "created_at", "data": "<JPEG en base64>" }] }
```

Le journal est rangé du plus ancien au plus récent. À la restauration, tout est vérifié avant d'écrire quoi que ce soit : format et version, identifiants, statuts, dates RFC 3339, types et longueur des entrées du journal. Chaque visage doit être un JPEG dont le SHA-256 correspond à son identifiant. Chaque légende repasse par le moteur de cohérence. Une légende déjà présente n'est remplacée que par une copie strictement plus récente (`updated_at`). Sinon elle est comptée dans `skipped`. Une légende dont le visage ne se trouve ni dans le fichier ni dans le coffre perd ce visage (`faces_missing`). Chaque légende ajoutée ou remplacée reçoit l'entrée de journal `imported from backup`.

## Visages

| Méthode | Route | Réponse |
|---|---|---|
| POST | `/api/faces/fetch?count=1..9` | `{faces:[{id,url,source}], requested}`. Renvoie 502 si la source est injoignable |
| POST | `/api/faces/upload` (corps : octets JPEG, PNG ou WebP, 8 Mo maximum) | 201 `{id,url}` |
| GET | `/api/faces/{id}/users` | ids des légendes qui utilisent ce visage |
| GET | `/faces/{id}.jpg` | l'image |

Pour attribuer un visage, renseigner `persona.face = {id, source}` puis envoyer la persona avec `POST /api/personas` ou `PUT /api/personas/{id}`. Un identifiant de visage inconnu est refusé avec une erreur 400.
