# API HTTP

Toutes les routes échangent du JSON. Les erreurs sont renvoyées sous la forme `{ "error": "…" }`, avec le code 400, 401, 404, 502 ou 500.

Si `SIMULACRE_TOKEN` est défini, chaque route `/api/*` et `/faces/*` exige `Authorization: Bearer <jeton>` ou le cookie `simulacre_token`. Seule `/api/health` reste publique.

## Catalogue et forge

| Méthode | Route | Corps / requête | Réponse |
|---|---|---|---|
| GET | `/api/health` | — | `{ok:true}` |
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
| GET | `/api/stats` | — | `{total, by_status, avg_coherence}` |
| GET | `/api/contamination` | — | `{pairs:[{a,b,risk,links}], names}` |

## Visages

| Méthode | Route | Réponse |
|---|---|---|
| POST | `/api/faces/fetch?count=1..9` | `{faces:[{id,url,source}], requested}`. Renvoie 502 si la source est injoignable |
| POST | `/api/faces/upload` (corps : octets JPEG, PNG ou WebP, 8 Mo maximum) | 201 `{id,url}` |
| GET | `/api/faces/{id}/users` | ids des légendes qui utilisent ce visage |
| GET | `/faces/{id}.jpg` | l'image |

Pour attribuer un visage, renseigner `persona.face = {id, source}` puis envoyer la persona avec `POST /api/personas` ou `PUT /api/personas/{id}`. Un identifiant de visage inconnu est refusé avec une erreur 400.
