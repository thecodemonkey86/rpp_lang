//! Zustand der Flussanalyse (Meilenstein M3): Typverengung (Kapitel 6), sichere
//! Initialisierung (R-4.46), Moves (Kapitel 5, R-5.3, R-5.15, R-5.16) und Loans
//! (Kapitel 5, Abschnitt 3.2).
//!
//! Der Zustand gilt an einer Stelle eines Methodenrumpfs. An Verzweigungen wird er
//! kopiert, an Zusammenführungen vereinigt: verengt bleibt, was auf allen Wegen verengt
//! ist; verschoben oder nicht zugewiesen ist, was es auf einem Weg ist. Ein Weg, der
//! nicht normal endet (`dead`), trägt nichts bei.

use std::collections::{BTreeMap, BTreeSet};

/// Nummer einer lokalen Variablen (eindeutig je Rumpf).
pub type LocalId = usize;

/// Wurzel `this` in einem Ort.
pub const THIS: LocalId = usize::MAX;

#[derive(Debug, Clone, Default)]
pub struct Flow {
    /// nicht erreichbar (nach `return`, `throw`, `break`, `continue`, `fail`)
    pub dead: bool,
    /// verengte nullbare Variablen (R-6.5)
    pub nonnull: BTreeSet<LocalId>,
    /// warum eine Variable nicht (mehr) verengt ist: die Regel der Stelle, an der der Fakt verloren ging
    pub lost: BTreeMap<LocalId, &'static str>,
    /// möglicherweise noch nicht zugewiesen (R-4.46)
    pub unassigned: BTreeSet<LocalId>,
    /// möglicherweise verschoben, mit der Regel, die den Move begründet
    pub moved: BTreeMap<LocalId, &'static str>,
}

impl Flow {
    pub fn dead() -> Flow {
        Flow { dead: true, ..Flow::default() }
    }

    /// Vereinigung zweier Stände. `null_rule` begründet den Verlust einer Verengung, die
    /// nur auf einem Weg galt; `move_rule` einen Move, der nur auf einem Weg geschah.
    pub fn join(&self, o: &Flow, null_rule: &'static str, move_rule: &'static str) -> Flow {
        if self.dead {
            return o.clone();
        }
        if o.dead {
            return self.clone();
        }
        let nonnull: BTreeSet<LocalId> = self.nonnull.intersection(&o.nonnull).copied().collect();
        let mut lost = o.lost.clone();
        lost.extend(self.lost.iter().map(|(k, v)| (*k, *v)));
        for x in self.nonnull.symmetric_difference(&o.nonnull) {
            lost.insert(*x, null_rule);
        }
        let mut moved = o.moved.clone();
        for (k, v) in &self.moved {
            moved.insert(*k, v);
        }
        for (k, v) in moved.iter_mut() {
            if self.moved.contains_key(k) != o.moved.contains_key(k) && *v == "R-5.3" {
                *v = move_rule;
            }
        }
        Flow { dead: false, nonnull, lost, unassigned: self.unassigned.union(&o.unassigned).copied().collect(), moved }
    }

    /// Gleiche Fakten (für den Fixpunkt an Schleifenköpfen).
    pub fn same(&self, o: &Flow) -> bool {
        self.dead == o.dead
            && self.nonnull == o.nonnull
            && self.unassigned == o.unassigned
            && self.moved.keys().eq(o.moved.keys())
    }

    pub fn narrow(&mut self, ids: &BTreeSet<LocalId>) {
        if !self.dead {
            self.nonnull.extend(ids.iter().copied());
        }
    }

    pub fn unnarrow(&mut self, id: LocalId, rule: &'static str) {
        if self.nonnull.remove(&id) || !self.lost.contains_key(&id) {
            self.lost.insert(id, rule);
        }
    }
}

/// Ein Ort (Kapitel 5, Abschnitt 3.1): eine Variable mit Feldnamen; `[]` steht für ein
/// Element mit unbekanntem Index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Place {
    pub root: LocalId,
    pub path: Vec<String>,
}

impl Place {
    pub fn var(root: LocalId) -> Place {
        Place { root, path: Vec::new() }
    }

    pub fn child(&self, name: &str) -> Place {
        let mut path = self.path.clone();
        path.push(name.to_string());
        Place { root: self.root, path }
    }

    fn prefix_of(&self, o: &Place) -> bool {
        self.path.len() <= o.path.len() && self.path.iter().zip(&o.path).all(|(a, b)| a == b)
    }
}

/// Wie lange ein Loan besteht.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Until {
    /// bis zum Ende des Blocks dieser Tiefe (ref-Variable, Schleife, lokaler Slice)
    Block(usize),
    /// bis zum Ende der Anweisung dieser Tiefe (nicht gebundene Borrow-Rückgabe, Slice)
    Stmt(usize),
    /// bis zum Ende dieses Aufrufs (Empfänger und Argumente, R-5.10)
    Call(usize),
}

/// Ein aktiver Borrow `(κ, p)` (Kapitel 5, Abschnitt 3.2).
#[derive(Debug, Clone)]
pub struct Loan {
    pub mutable: bool,
    pub place: Place,
    /// Die Wurzel ist eine shared-Variable: Dann überlappen alle Orte mit dieser Wurzel.
    pub shared_root: bool,
    pub until: Until,
    /// Regel, gegen die ein Zugriff während des Loans verstößt
    pub rule: &'static str,
    /// Name des Orts für Meldungen
    pub what: String,
}

impl Loan {
    /// `erlaubt_L(κ, p)` für diesen einen Loan.
    pub fn conflicts(&self, mutable: bool, p: &Place) -> bool {
        if !mutable && !self.mutable {
            return false;
        }
        p.root == self.place.root && (self.shared_root || p.prefix_of(&self.place) || self.place.prefix_of(p))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(xs: &[usize]) -> BTreeSet<usize> {
        xs.iter().copied().collect()
    }

    #[test]
    fn vereinigung() {
        let mut a = Flow { nonnull: set(&[1, 2]), ..Flow::default() };
        a.moved.insert(3, "R-5.3");
        let b = Flow { nonnull: set(&[2]), unassigned: set(&[4]), ..Flow::default() };
        let j = a.join(&b, "R-6.8", "R-5.15");
        assert_eq!(j.nonnull, set(&[2]));
        assert_eq!(j.lost.get(&1), Some(&"R-6.8"));
        assert_eq!(j.moved.get(&3), Some(&"R-5.15"));
        assert_eq!(j.unassigned, set(&[4]));
        let d = Flow::dead();
        assert!(d.join(&a, "", "").same(&a));
        assert!(a.join(&d, "", "").same(&a));
    }

    #[test]
    fn ueberlappung() {
        let x = Place::var(1);
        let l = Loan { mutable: true, place: x.child("left"), shared_root: false, until: Until::Call(0), rule: "", what: String::new() };
        assert!(!l.conflicts(true, &x.child("right")));
        assert!(l.conflicts(false, &x));
        assert!(l.conflicts(false, &x.child("left").child("v")));
        let s = Loan { shared_root: true, ..l.clone() };
        assert!(s.conflicts(true, &x.child("right")));
        let r = Loan { mutable: false, ..l };
        assert!(!r.conflicts(false, &x.child("left")));
        assert!(r.conflicts(true, &x));
    }
}
