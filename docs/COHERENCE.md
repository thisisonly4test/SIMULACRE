# Moteur de cohérence

Chaque contrôle correspond à une question qu'un interlocuteur attentif, ou l'équipe de modération d'une plateforme, pourrait se poser. Le moteur (`coherence.rs`) tourne après chaque génération et après chaque modification.

## Score

`score = 100 − 25 × erreurs − 8 × attentions − 2 × infos`, borné entre 0 et 100.

Sur 600 légendes générées (4 pays × 13 archétypes), le test `corpus_has_no_coherence_errors` exige **zéro erreur**, et le test `corpus_scores_are_high` exige un score moyen d'au moins 80.

## Contrôles

| Domaine | Contrôle | Sévérité |
|---|---|---|
| Identité | âge = âge calculé depuis la date de naissance | erreur |
| | moins de 18 ans | erreur |
| | prénom ou nom vide | erreur |
| Études | commencées avant 10 ans, ou fin avant le début | erreur |
| | étape commencée avant la fin de la précédente | attention |
| | diplôme obtenu dans le futur | erreur |
| Carrière | emploi commencé avant 16 ans | erreur |
| | fin avant le début, ou début dans le futur | erreur |
| | emploi principal commencé pendant les études | info |
| | deux emplois successifs qui se chevauchent | attention |
| Statut | en activité sans exactement un poste en cours | erreur |
| | retraité avec un poste en cours | erreur |
| | retraité avant 55 ans | attention |
| | étudiant sans formation en cours | erreur |
| Logement | emménagement avant la naissance ou dans le futur | erreur |
| | propriétaire avant 23 ans | attention |
| Famille | parent avant 16 ans | erreur |
| | parent avant 19 ans | attention |
| | enfant né dans le futur | erreur |
| | enfant portant le prénom de la légende | info |
| | plus de 15 ans d'écart avec le partenaire | attention |
| | partenaire mineur | erreur |
| | parent de la légende avant 15 ans | erreur |
| | animal adopté avant la naissance ou dans le futur | erreur |
| Comptes | âge inférieur au minimum de la plateforme | erreur |
| | pseudo non conforme (longueur, caractères, casse), avec une correction proposée | erreur |
| | compte déclaré « depuis » une année où la plateforme n'était pas encore répandue | erreur |
| | compte ouvert avant l'âge minimum, ou dans le futur | erreur |
| | très actif sur une plateforme atypique pour son âge (écart supérieur à 2,2) | info |
| Exposition | pseudos qui partagent une sous-chaîne d'au moins 5 caractères | info |
| | année de naissance visible dans un pseudo | info |
| Rythme | activité moyenne au-dessus de 45 % pendant les heures de bureau | attention |
| | fuseau horaire absent du pays | attention |
| Visage | aucun visage attribué | info |

## Exposition (0–100)

`60 × (part des comptes reliés par une sous-chaîne commune) + 8 × (comptes contenant l'année de naissance) + 6 × (comptes sous le nom réel)`

Les vraies personnes réutilisent leurs pseudos. Le générateur reproduit ce comportement, avec une probabilité qui augmente avec la jeunesse et une faible conscienciosité. La métrique rend visible ce que ce réalisme coûte.

## Contamination entre légendes

`contamination.rs` compare toutes les paires de légendes du coffre, sauf les légendes grillées.

| Recoupement | Sévérité |
|---|---|
| même visage (hash) | erreur |
| même nom complet | erreur |
| même téléphone | erreur |
| même identifiant d'e-mail | erreur |
| pseudo identique une fois normalisé | erreur |
| pseudos proches (sous-chaîne commune ≥ 6) | attention |
| même date de naissance | attention |
| même quartier | attention |
| employeur commun | attention |
| bio identique | attention |
| rythme en ligne quasi identique (corrélation de Pearson > 0,93) | attention |
| même nom de famille, même animal, même ville et trois loisirs communs | info |

`risque = Σ (45 / 18 / 6)`, plafonné à 100.

Le contrôle du **rythme** est le plus original. Un opérateur qui fait vivre deux légendes depuis le même bureau les fait poster aux mêmes heures. La corrélation de leurs cartes 7×24 révèle cette signature.
