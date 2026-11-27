//! Gemeinsamer Zustand aller Threads eines Programms (Kapitel 16).
//!
//! Jeder R++-Thread ist ein Thread des Betriebssystems mit einem eigenen `Interp`. Die Werte
//! des Interpreters (`Rc`, `RefCell`) sind nicht threadfest; deshalb führt immer nur der Thread
//! R++-Code aus, der die Sperre der `World` hält. Abgegeben wird sie beim Warten (`join`,
//! `send`, `receive`, Bereichsende, `sleep`, Eingabe) und regelmäßig an den Abbruchstellen
//! (R-16.6), sodass kein Thread die anderen auf Dauer aufhält (R-16.23). Alles in `State` wird
//! nur mit gehaltener Sperre gelesen und geschrieben.

use crate::console::Console;
use crate::literals::FileLiterals;
use crate::save_file::SaveFile;
use crate::value::{Obj, Value};
use std::cell::UnsafeCell;
use std::collections::{HashMap, HashSet, VecDeque};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Condvar, Mutex};
use std::time::Duration;

struct Gil {
    /// nächste Wartenummer
    next: u64,
    /// Nummer, die die Sperre hält oder als nächste bekommt
    serving: u64,
    /// zählt jede Änderung, auf die ein Thread warten kann
    generation: u64,
}

pub(crate) struct ThreadRec {
    pub done: bool,
    /// Ergebnis oder Exception mit Stacktrace; `None` nach einem Programmfehler oder Abbruch
    pub result: Option<Result<Value, (Value, Vec<String>)>>,
    pub joined: bool,
}

pub(crate) struct Chan {
    pub queue: VecDeque<Value>,
    pub cap: usize,
    pub senders: usize,
    pub receiver: bool,
}

pub(crate) struct FirstFatal {
    pub tid: i64,
    pub name: &'static str,
    pub msg: String,
    pub trace: Vec<String>,
}

pub(crate) struct State {
    pub console: Console,
    pub save_files: HashMap<*const Obj, SaveFile>,
    pub text_readers: HashMap<*const Obj, std::io::BufReader<std::fs::File>>,
    /// geladene C-Bibliotheken (Kapitel 15)
    pub natives: Option<Rc<crate::ffi::Natives>>,
    pub literals: Vec<FileLiterals>,
    pub threads: HashMap<i64, ThreadRec>,
    /// Threads je Bereich in Startreihenfolge (R-16.11)
    pub scopes: HashMap<i64, Vec<i64>>,
    pub channels: HashMap<i64, Chan>,
    pub next_id: i64,
    /// laufende Threads einschließlich main
    pub alive: usize,
    /// wartende Threads mit dem Stand, bei dem sie ihre Bedingung zuletzt geprüft haben (R-16.19)
    pub waiting: HashMap<i64, u64>,
    /// Threads, die auf Eingabe warten (R-16.6)
    pub external: HashSet<i64>,
    /// der erste Programmfehler (R-16.6)
    pub first_fatal: Option<FirstFatal>,
    /// rpp.sync wurde benutzt: Die Prüfung der Invarianten (Prüfmodus) gilt nur ohne Threads und Kanäle.
    pub sync_used: bool,
}

pub(crate) struct World {
    gil: Mutex<Gil>,
    cv: Condvar,
    /// Es gab je mehr als einen Thread: Abbruchstellen und Abgabe der Sperre sind aktiv.
    pub active: AtomicBool,
    /// Ein Programmfehler bricht alle Threads ab (R-16.6).
    pub cancel: AtomicBool,
    st: UnsafeCell<State>,
}

// Zugriff auf `st` nur mit gehaltener Sperre (siehe oben).
unsafe impl Sync for World {}
unsafe impl Send for World {}

/// Trägt Werte des Interpreters in einen neuen Thread; benutzt werden sie dort nur mit der Sperre.
pub(crate) struct SendBox<T>(pub T);
unsafe impl<T> Send for SendBox<T> {}

impl World {
    pub fn new(console: Console, literals: Vec<FileLiterals>) -> &'static World {
        Box::leak(Box::new(World {
            gil: Mutex::new(Gil { next: 0, serving: 0, generation: 0 }),
            cv: Condvar::new(),
            active: AtomicBool::new(false),
            cancel: AtomicBool::new(false),
            st: UnsafeCell::new(State {
                console, save_files: HashMap::new(), text_readers: HashMap::new(), natives: None, literals,
                threads: HashMap::new(), scopes: HashMap::new(), channels: HashMap::new(), next_id: 1, alive: 1,
                waiting: HashMap::new(), external: HashSet::new(), first_fatal: None, sync_used: false,
            }),
        }))
    }

    /// Der gemeinsame Zustand; nur mit gehaltener Sperre aufrufen.
    #[allow(clippy::mut_from_ref)]
    pub fn st(&'static self) -> &'static mut State {
        unsafe { &mut *self.st.get() }
    }

    pub fn is_active(&self) -> bool {
        self.active.load(Ordering::Relaxed)
    }

    pub fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }

    fn enter(&self, mut g: std::sync::MutexGuard<'_, Gil>) {
        let ticket = g.next;
        g.next += 1;
        while g.serving != ticket {
            g = self.cv.wait(g).unwrap();
        }
    }

    /// Sperre nehmen (in der Reihenfolge der Anfragen).
    pub fn acquire(&self) {
        let g = self.gil.lock().unwrap();
        self.enter(g);
    }

    pub fn release(&self) {
        let mut g = self.gil.lock().unwrap();
        g.serving += 1;
        self.cv.notify_all();
    }

    /// Die Sperre kurz abgeben, damit andere Threads laufen.
    pub fn yield_now(&self) {
        let mut g = self.gil.lock().unwrap();
        if g.next == g.serving + 1 {
            return; // niemand wartet
        }
        g.serving += 1;
        self.cv.notify_all();
        self.enter(g);
    }

    pub fn generation(&self) -> u64 {
        self.gil.lock().unwrap().generation
    }

    /// Etwas hat sich geändert, worauf ein Thread warten kann.
    pub fn bump(&self) {
        let mut g = self.gil.lock().unwrap();
        g.generation += 1;
        self.cv.notify_all();
    }

    /// Sperre abgeben, warten, bis sich seit `seen` etwas geändert hat (höchstens `timeout`),
    /// und die Sperre wieder nehmen.
    pub fn wait_change(&self, seen: u64, timeout: Option<Duration>) {
        let mut g = self.gil.lock().unwrap();
        g.serving += 1;
        self.cv.notify_all();
        let start = std::time::Instant::now();
        while g.generation == seen {
            match timeout {
                None => g = self.cv.wait(g).unwrap(),
                Some(t) => {
                    let Some(rest) = t.checked_sub(start.elapsed()) else { break };
                    g = self.cv.wait_timeout(g, rest).unwrap().0;
                }
            }
        }
        self.enter(g);
    }
}
