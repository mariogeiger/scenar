# Scenar — décisions de conception

Issues d'une séance de « grilling » entre Robin et Claude, le 2026-10-20 : trois tours de questions, chacune avec une recommandation que Robin a acceptée ou modifiée.
Plusieurs décisions ont été ajoutées ou changées après la v1 : elles sont dans la dernière section.
Ces décisions appartiennent à Robin. Ne les change pas sans lui demander.

## Tour 1 — le cadre

| # | Question | Options envisagées | Décision | Pourquoi |
|---|---|---|---|---|
| 1 | Pour qui ? | Soi seul · petite équipe · produit public | **Robin seul, en local** | Ni comptes ni collaboration à gérer |
| 2 | Sous quelle forme ? | Page web · app de bureau · plugin (Obsidian, Notion…) · ligne de commande | **Web, mais toute la stack en Rust** | Choix de Robin (la recommandation était du HTML/JS simple) |
| 3 | Qu'est-ce qu'un bloc ? | Scène · beat · séquence/acte | **Blocs imbriqués, jusqu'à la scène puis au beat** | Choix de Robin. Les niveaux exacts ont été fixés au tour 2 |
| 4 | Comment naît une dépendance ? | À la main · par éléments tagués · détection automatique | **Par éléments, plus une détection automatique, « comme une déclaration de variable dans le code »** | Choix de Robin. La syntaxe a été fixée au tour 2 |
| 5 | Les dépendances ont-elles un sens ? | Lien simple · setup → payoff | **Setup → payoff**, avec d'autres types à brainstormer | Permet des warnings dans les deux sens (pistolet jamais utilisé / pistolet sorti de nulle part) |
| 6 | L'ordre chronologique compte-t-il ? | Oui · non | **Oui** | Déplacer un payoff avant son setup doit être signalé |
| 7 | Que faire quand on supprime une scène ? | Warnings persistants · confirmation · les deux | **Les deux, plus un archivage et une timeline avec une bin (existe / n'existe pas)** | Idée de Robin : la bin, comme en montage vidéo |
| 8 | Format du texte | Fountain · texte libre | **Texte libre dans les beats** pour l'instant, le format viendra plus tard | Ne pas bloquer la v1 sur la mise en forme |
| 9 | Sauvegarde | Navigateur seul · fichier JSON · un fichier par scène | **Export/import JSON + sauvegarde automatique dans le navigateur** | Portable, et rien ne se perd si on vide le cache |

## Tour 2 — technique et syntaxe (Robin : « toutes tes recos »)

| # | Question | Décision |
|---|---|---|
| 1 | Où tourne le Rust ? | **Tout dans le navigateur** : Leptos compilé en WASM. Nécessite rustup au lieu du Rust de Homebrew. Rejeté : un serveur Axum avec une interface en JS, ou un moteur WASM avec une interface en JS |
| 2 | Niveaux d'imbrication | **Fixes : Acte → Scène → Beat.** Le texte et les éléments vivent dans les beats ; les niveaux au-dessus servent à regrouper |
| 3 | Syntaxe « variable » | `@pistolet` **déclare** l'élément (setup) · `!pistolet` marque le **payoff** · les mentions simples sont **détectées automatiquement**. Les warnings imitent un compilateur : élément déclaré mais jamais payé, utilisé avant d'être déclaré, payé sans avoir été déclaré |
| 4 | Alias | Déclarés une fois, avec l'élément : `@pistolet(flingue, arme)`. La détection des mentions vagues par IA viendra plus tard |
| 5 | Bin | Un bloc archivé est **conservé et restaurable, mais compte comme inexistant** pour les dépendances. La suppression définitive se fait seulement depuis la bin, avec confirmation |
| 6 | Autres types de dépendances | **v1 = setup/payoff seulement.** Le moteur est pensé pour accueillir plus tard : introduction de personnage, savoir, question → réponse, état (objet détruit qui réapparaît), callback. Les plus précieux ensuite : savoir et état |
| 7 | Sauvegarde, une fois Rust choisi | Fichier JSON, même format dans tous les cas |

## Tour 3 — interface (Robin : « oui faisons comme ça »)

| # | Question | Décision |
|---|---|---|
| 1 | Disposition | 3 colonnes : timeline en arbre à gauche (bin en bas) · scène sélectionnée au centre, en pile de beats · éléments et warnings à droite |
| 2 | Affichage des dépendances | Un **champ texte simple, avec des pastilles sous chaque beat** (setup / mention / payoff). Cliquer sur une pastille surligne les beats liés dans la timeline. La coloration directement dans le texte viendra plus tard |
| 3 | Détection des mentions | Mot entier, sans tenir compte des majuscules, accents pris en compte, **pas de gestion des pluriels** : les variantes s'ajoutent en alias |
| 4 | Archivage | Possible à tous les niveaux, les enfants suivent. Une confirmation liste les dépendants avant d'archiver |
| 5 | Toolchain | Désinstaller le Rust de Homebrew, installer rustup avec la cible `wasm32-unknown-unknown` et trunk |
| 6 | Framework | **Leptos en rendu côté client.** Le moteur de dépendances est un module Rust pur, testé séparément (`src/engine.rs`) |
| 7 | Emplacement | `~/Documents/projects/scenar` |

## Ajouts et changements après la v1

- **Warnings regroupés dans un onglet.** Robin : « enlever les alertes et juste mettre des warnings dans un autre onglet ». Les warnings n'apparaissent plus que dans l'onglet « Warnings » du panneau de droite : plus de badges rouges dans la timeline ni de pastilles rouges. La confirmation avant archivage est conservée.
- **Monter / descendre les blocs** avec des boutons ↑↓ dans la timeline et sur chaque beat. Le glisser-déposer place désormais le bloc avant ou après la ligne visée, selon la moitié où on le lâche, et une ligne bleue montre où il va atterrir. Les blocs archivés (cachés) sont sautés.
- **Design « bureau d'écrivain »** : papier chaud, serif de manuscrit pour le texte, accent terracotta, beats numérotés dans la marge comme dans un script. Modes clair et sombre.
- **Port fixé à 8091** dans `Trunk.toml`, pour éviter le conflit avec Docker sur 8090 et garder la même sauvegarde d'une session à l'autre.

## Mise en ligne sur scenar.geiger.ink (Mario, 2026-10-20)

Une séance de « grilling » avec Mario, à partir de deux maquettes. Les décisions 1 à 7 s'ajoutent à celles de Robin ; les 8 et 9 en remplacent deux, à la demande de Mario.

| # | Question | Décision |
|---|---|---|
| 1 | D'où part le code ? | **De zéro**, d'après ce fichier et la maquette simple |
| 2 | Où vivent les données ? | **Dans le navigateur de chacun**, avec l'export/import JSON. Le site est statique, servi par Caddy |
| 3 | Accès | **Ouvert à tous** : le serveur ne stocke rien |
| 4 | Interface | **Le look épuré de la maquette simple, avec toutes les fonctions décidées, discrètes** : les beats ne se déplient dans l'arbre que pour la scène ouverte, les ↑↓ apparaissent au survol |
| 5 | Code et déploiement | Repo `~/git/scenar` sur rtx-home, sans remote ; `deploy.sh` publie `dist/` dans `~/server/scenar/` |
| 6 | Première ouverture | **Un seul scénario par navigateur**, prérempli avec l'exemple « Le Phare de Kerlaz » ; « Nouveau » le vide après confirmation |
| 7 | Règles du moteur | Les trois de Robin, plus **« déclaré deux fois »** quand un élément a un second `@` |
| 8 | Mise en page, après quatre maquettes | **Deux colonnes et des notes en marge** : la timeline à gauche ; chaque beat porte dans sa marge droite ses éléments et l'autre bout de leur dépendance (« → payé en II.1.2 », « ⚠ jamais payé ») ; les éléments et les warnings tiennent sur une ligne en bas de page, qui déplie la liste des éléments. **Remplace les 3 colonnes du tour 3 #1 et les pastilles du tour 3 #2** |
| 9 | Style, après quatre maquettes | **Machine à écrire** : Courier Prime partout (hébergée sur le site, licence OFL), papier blanc, encre noire, accent rouge ; mode sombre conservé. **Remplace le design « bureau d'écrivain »** |
| 10 | Interactions, après les retours de Mario | **Rien ne bouge sous le pointeur** : le survol ne fait que teinter, jamais changer une taille ou une position. **Une grammaire visible** : les capitales grises sont des titres, l'encre ouvre un lieu de l'histoire, les `[crochets]` sont des boutons, `⋯` ouvre les actions d'un bloc. **Rien ne se déroule sur place** : la bin, le panneau des éléments et warnings, les actions d'un bloc et, sur téléphone, la timeline s'ouvrent au clic, par-dessus la page. La timeline ne liste plus que les actes et les scènes. **Remplace la décision 4 et le déroulé de la décision 8** |
| 11 | Largeurs, parmi quatre maquettes | **Trois idées combinées.** Le script se lit d'un seul tenant, tous les actes et scènes à la suite, et la scène lue est suivie au défilement. Une frise des scènes, en haut, relie par des arcs chaque setup à ses payoffs. Des colonnes s'ajoutent avec la largeur : sur téléphone, le script seul ; sur desktop, la timeline à côté ; dès 1400 px, le panneau des éléments et warnings en colonne ; dès que deux pages tiennent, le script se met en pages côte à côte, autant qu'il en tient. La taille du texte suit la largeur |
| 12 | Publication du code | **Public sur GitHub**, `github.com/mariogeiger/scenar` ; remplace le « sans remote » de la décision 5 |
| 13 | Retour visuel (20 octobre 2026) | **La frise est une bande partitionnée trois fois** : actes, scènes, beats, chaque beat aussi large que son texte (avec un minimum), chaque bloc la somme de ce qu'il contient ; un beat en warning est rouge ; les arcs pendent sous la bande, de beat à beat. **La sélection est un chemin acte ⊃ scène ⊃ beat** : un clic dans le texte ou sur une case de la frise la pose (la frise défile sans mettre le curseur) ; un même gris la montre dans la frise et la timeline, et des barres emboîtées dans la marge gauche. Le défilement ne la reprend que lorsqu'elle sort de l'écran. **Survoler un arc** le rougit, teinte ses deux beats et leurs notes, et surligne les mots `@nom` / `!nom` ; un clic fixe ce surlignage. **Survoler un beat** teinte sa case et rougit ses arcs. Complète la décision 11 |
| 14 | Liens partagés et édition simultanée (20 octobre 2026) | **[Sauver] met le brouillon sur le serveur sous un lien** `scenar.geiger.ink/<id>`, l'id faisant 16 caractères `a-z A-Z 0-9` tirés au hasard (≈ 95 bits) ; `/` repart alors sur un brouillon vierge, et « Mes liens » rappelle les liens ouverts dans ce navigateur. Sur `/`, rien ne part au serveur ; **sur un lien, tout se sauve tout seul**. Qui a le lien peut lire et modifier. **L'édition simultanée passe par un CRDT complet (Yrs)** : textes fusionnés lettre à lettre ; place d'un bloc = registres parent + rang, le dernier qui écrit gagne, donc jamais de doublon. Les curseurs des autres s'affichent en couleur, nom facultatif montré au survol ; une ligne dit combien sont là et l'état de la synchro. Sur un lien, [Nouveau] ramène à `/` sur un brouillon vierge, [Importer] remplace le contenu pour tous après confirmation, [Exporter] ne change pas. Limites du serveur : 2 Mo par scénario, 20 créations par heure et par IP, pas d'expiration. Remplace, pour les liens, les décisions 2 et 3 (le serveur stocke désormais les scénarios partagés) |
