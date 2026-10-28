# Scenar

Un atelier d'écriture de scénario qui vérifie les dépendances comme un
compilateur : `@pistolet` déclare un élément (setup), `!pistolet` le paie
(payoff), et chaque autre occurrence du nom ou d'un alias est une mention.
La page est en Rust compilé en WASM avec Leptos. Sur `/`, le scénario est un
brouillon qui ne vit que dans le navigateur ; [Sauver] le met sur le serveur
sous un lien `/<16 caractères>`, que tous ceux qui l'ont éditent ensemble,
en temps réel, grâce au CRDT Yrs.

- `src/engine.rs` : le moteur de dépendances et ses quatre règles.
- `src/model.rs` : l'arbre Acte → Scène → Beat, l'archivage, le format JSON.
- `src/shared.rs`, `src/rank.rs` : le scénario partagé, document Yrs, et
  l'ordre des blocs frères.
- `src/example.rs` : le scénario d'exemple de la première visite.
- `src/state.rs`, `src/link.rs`, `src/dnd.rs`, `src/storage.rs`, `src/ui/` :
  la page ; `fonts/` : Courier Prime (OFL).
- `server/` : `scenar-server`, qui garde les scénarios partagés et relaie
  les changements entre navigateurs (protocole de synchro Yjs).
- `DECISIONS.md` : les décisions de conception.

```
cargo test --workspace   # le moteur, le modèle, le document partagé, le serveur
trunk serve              # la page seule, http://localhost:8091
~/git/geiger-server/deploy scenar   # publie sur https://scenar.geiger.ink
```

En production, Caddy sert `dist/` (toute adresse inconnue rend
`index.html`) et passe `/api/*` à `scenar-server` (unité systemd
utilisateur `geiger-scenar`, port 4010, scénarios dans
`~/server/data/scenar/`).

## Licence

Le code est sous licence MIT (voir `LICENSE`). La police Courier Prime,
dans `fonts/`, reste sous sa propre licence SIL OFL 1.1 (`fonts/OFL.txt`).
