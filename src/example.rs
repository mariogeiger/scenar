//! The story a first visit opens on: short, but it shows the syntax, an
//! alias, the bin and each kind of warning a story usually meets.

use crate::model::{FORMAT, Node, Story};

pub fn kerlaz() -> Story {
    let mut next = 0;
    let mut id = || {
        next += 1;
        next
    };
    let mut beat =
        |text: &str| Node { id: id(), title: String::new(), text: text.into(), archived: false, children: vec![] };
    let i1 = [
        beat(
            "Léa descend du bateau sous la pluie. Sur le quai, une femme en ciré jaune l'attend : @Anne(la voisine), qui gardait le phare depuis la mort du père.",
        ),
        beat("Anne lui tend un trousseau. « Il parlait de toi tous les soirs. » Léa ne répond pas."),
    ];
    let i2 = [
        beat(
            "Léa ouvre le tiroir du buffet. Sous les nappes, un @pistolet(flingue, arme) enveloppé dans un chiffon huilé. Elle le repose sans un mot.",
        ),
        beat("Sur la table, une @lettre adressée à son père, jamais ouverte. Le cachet date de la nuit du naufrage."),
        beat("Un bruit au grenier. Léa repense au pistolet. Elle monte quand même, les mains vides."),
    ];
    let flashback = [beat("1987. Le père, à vingt ans, allume le phare pour la première fois.")];
    let i3 = [beat(
        "Anne sert la soupe. Le père n'allumait plus le phare les soirs de tempête, dit-elle. Dans un coin, une vieille @boussole marine que Léa reconnaît.",
    )];
    let ii1 = [
        beat("La tempête se lève. Dans la salle des machines, la radio grésille des appels que personne ne prend."),
        beat(
            "Un homme l'attend en haut des marches. Léa sort l'!arme du buffet et la pointe vers lui. C'est le fils d'!Anne.",
        ),
    ];
    let ii2 = [
        beat("Léa rallume la vieille @radio de son père et capte un signal de détresse."),
        beat("Grâce à la !radio, les secours trouvent le chalutier avant l'aube."),
    ];
    let iii1 = [
        beat("Au matin, Léa ouvre enfin la !lettre. Son père savait. Il avait tout écrit."),
        beat("Elle referme le phare avec la !clé du père et rend le trousseau à Anne."),
    ];
    let mut node = |title: &str, archived: bool, children: Vec<Node>| Node {
        id: id(),
        title: title.into(),
        text: String::new(),
        archived,
        children,
    };
    let acts = vec![
        {
            let scenes = vec![
                node("Le quai", false, i1.into()),
                node("La maison du gardien", false, i2.into()),
                node("Flashback 1987", true, flashback.into()),
                node("Dîner chez Anne", false, i3.into()),
            ];
            node("L'arrivée", false, scenes)
        },
        {
            let scenes = vec![node("Le phare éteint", false, ii1.into()), node("Le naufrage", false, ii2.into())];
            node("La tempête", false, scenes)
        },
        {
            let scenes = vec![node("L'aube", false, iii1.into())];
            node("La marée", false, scenes)
        },
    ];
    Story { format: FORMAT, title: "Le Phare de Kerlaz".into(), acts }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{Rule, analyze};
    use crate::model::Level;

    #[test]
    fn kerlaz_shows_three_warnings_and_round_trips() {
        let s = kerlaz();
        let texts: Vec<String> =
            s.live().iter().filter(|l| l.level == Level::Beat).map(|l| s.find(l.id).unwrap().text.clone()).collect();
        let a = analyze(texts.iter().map(String::as_str));
        let found: Vec<(Rule, &str)> =
            a.warnings.iter().map(|w| (w.rule, a.elements[w.element].name.as_str())).collect();
        assert_eq!(
            found,
            [(Rule::NeverPaid, "boussole"), (Rule::UsedBeforeDeclared, "radio"), (Rule::PaidUndeclared, "clé")]
        );
        assert_eq!(Story::from_json(&s.to_json()).unwrap(), s);
    }
}
