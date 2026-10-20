# Scenar

Un atelier d'écriture de scénario qui vérifie les dépendances comme un
compilateur : `@pistolet` déclare un élément (setup), `!pistolet` le paie
(payoff), et chaque autre occurrence du nom ou d'un alias est une mention.
Tout tourne dans le navigateur, en Rust compilé en WASM avec Leptos ; le
scénario se sauve dans le navigateur et s'exporte en JSON.

- `src/engine.rs` : le moteur de dépendances et ses quatre règles.
- `src/model.rs` : l'arbre Acte → Scène → Beat, l'archivage, le format JSON.
- `src/example.rs` : le scénario d'exemple de la première visite.
- `src/state.rs`, `src/dnd.rs`, `src/storage.rs`, `src/ui/` : la page ; `fonts/` : Courier Prime (OFL).
- `DECISIONS.md` : les décisions de conception.

```
cargo test --lib     # le moteur et le modèle
trunk serve          # http://localhost:8091
./deploy.sh          # publie sur https://scenar.geiger.ink
```

## Licence

Le code est sous licence MIT (voir `LICENSE`). La police Courier Prime,
dans `fonts/`, reste sous sa propre licence SIL OFL 1.1 (`fonts/OFL.txt`).
