//! rpp.sync (Kapitel 16): Threads, Bereiche und Kanäle im Interpreter.
//!
//! Ein R++-Thread läuft in einem eigenen Thread des Betriebssystems mit einem eigenen `Interp`;
//! R++-Code führt immer nur der Thread aus, der die Sperre der `World` hält (siehe `world.rs`).
//! Die Objekte `Thread`, `Task`, `Scope`, `Sender` und `Receiver` tragen in ihrem Feld `id`
//! die Nummer des Eintrags in der `World`.

use super::*;
use crate::world::{Chan, SendBox, State, ThreadRec};
use std::collections::VecDeque;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

fn obj_id(o: &ORef) -> i64 {
    match o.slots.borrow().first() {
        Some(Value::I64(v)) => *v,
        _ => 0,
    }
}

impl Interp {
    fn thread_error(&self, msg: impl Into<String>) -> Flow {
        self.fatal("ThreadError", msg)
    }

    /// Wartet mit abgegebener Sperre, bis `ready` gilt (R-16.19: Warten alle Threads, ohne dass
    /// sich noch etwas ändern kann, ist das ein Programmfehler). Ein Thread, der schon wegen
    /// eines Programmfehlers abgewickelt wird, wartet ohne weitere Fehler.
    fn wait_until(&mut self, ready: impl Fn(&State) -> bool) -> R<()> {
        loop {
            if ready(self.w.st()) {
                return Ok(());
            }
            let unwinding = self.fatals.get() > 0;
            if self.w.cancelled() && !unwinding {
                return Err(self.cancelled());
            }
            let seen = self.w.generation();
            let st = self.w.st();
            if !self.w.cancelled() {
                st.waiting.insert(self.tid, seen);
                if st.waiting.len() >= st.alive && st.waiting.values().all(|g| *g == seen) {
                    st.waiting.remove(&self.tid);
                    // welcher Thread die Verklemmung bemerkt, ist Zufall: ohne Stacktrace
                    let mut f = self.thread_error("Deadlock: every thread is waiting for another one");
                    if let Flow::Fatal(fatal) = &mut f {
                        fatal.trace.clear();
                    }
                    if let Some(first) = self.w.st().first_fatal.as_mut().filter(|first| first.tid == self.tid) {
                        first.trace.clear();
                    }
                    return Err(f);
                }
            }
            self.w.wait_change(seen, None);
            self.w.st().waiting.remove(&self.tid);
        }
    }

    /// Ist der Thread fertig? Beim Abwickeln nach einem Programmfehler gilt auch ein Thread als
    /// fertig, der auf Eingabe wartet (R-16.6).
    fn thread_done(st: &State, id: i64, unwinding: bool) -> bool {
        st.threads.get(&id).is_none_or(|t| t.done) || (unwinding && st.external.contains(&id))
    }

    /// Startet einen Thread, der `body` ausführt (R-16.2); liefert seine Nummer.
    fn start_thread(&mut self, body: Rc<Closure>, input: Option<Value>) -> R<i64> {
        let w = self.w;
        let st = w.st();
        let id = st.next_id;
        st.next_id += 1;
        st.threads.insert(id, ThreadRec { done: false, result: None, joined: false });
        st.alive += 1;
        w.active.store(true, Ordering::Relaxed);
        let (p, release) = (self.p, self.release);
        let pack = SendBox((body, input));
        let spawned = std::thread::Builder::new().stack_size(2 << 30).spawn(move || {
            let pack = pack;
            let (body, input) = pack.0;
            w.acquire();
            let run = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let mut it = Interp::new(p, release, w, id);
                let mut args: Vec<Value> = input.into_iter().collect();
                let r = it.call_closure(&body, &mut args);
                let d = it.drop_value(Value::Func(body));
                match (r, d) {
                    (Err(Flow::Fatal(f)), _) | (_, Err(Flow::Fatal(f))) => Err(Flow::Fatal(f)),
                    (r, _) => r,
                }
            }));
            let st = w.st();
            let result = match run {
                Ok(Ok(v)) => Some(Ok(v)),
                Ok(Err(Flow::Throw(ex, trace))) => Some(Err((ex, trace))),
                Ok(Err(Flow::Fatal(_))) => None,
                Ok(Err(_)) => Some(Ok(Value::Void)),
                Err(e) => {
                    let msg = e.downcast_ref::<String>().cloned().or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string())).unwrap_or_default();
                    if st.first_fatal.is_none() {
                        st.first_fatal = Some(world::FirstFatal { tid: id, name: "InternalError", msg, trace: Vec::new() });
                    }
                    w.cancel.store(true, Ordering::Relaxed);
                    None
                }
            };
            if let Some(t) = st.threads.get_mut(&id) {
                t.result = result;
                t.done = true;
            }
            st.alive -= 1;
            st.waiting.remove(&id);
            w.bump();
            w.release();
        });
        if let Err(e) = spawned {
            let st = self.w.st();
            st.threads.remove(&id);
            st.alive -= 1;
            return Err(self.thread_error(format!("The thread cannot be started: {e}")));
        }
        Ok(id)
    }

    /// Ende von `main` mit Threads (R-16.6, R-16.20): warten, bis alle Threads geendet haben;
    /// nach einem Programmfehler in einem anderen Thread dessen Meldung und Exit-Code 2.
    pub(crate) fn end_threads(&mut self, r: &R<Value>) -> Option<i32> {
        loop {
            let st = self.w.st();
            // Nur nach einem Programmfehler bleibt ein Thread zurück, der auf Eingabe wartet
            // oder in C-Code steckt (R-16.6); sonst wartet main auf alle (R-16.20).
            let stuck = if self.w.cancelled() { st.external.iter().filter(|t| **t != self.tid).count() } else { 0 };
            if st.alive <= 1 + stuck {
                break;
            }
            let seen = self.w.generation();
            self.w.wait_change(seen, Some(Duration::from_millis(200)));
        }
        let st = self.w.st();
        let first = st.first_fatal.as_ref()?;
        if first.tid == self.tid && matches!(r, Err(Flow::Fatal(_))) {
            return None;
        }
        if first.name == "InvariantViolation" {
            st.console.write_stderr(&format!("Internal error (invariant checking): {}\n", first.msg));
            return Some(3);
        }
        st.console.write_stderr(&format!("{}: {}\n", first.name, first.msg));
        if !self.release {
            for t in &first.trace {
                st.console.write_stderr(&format!("    at {t}\n"));
            }
        }
        Some(2)
    }

    /// `Console::readLine` mit mehreren Threads: Die Sperre ist während des Lesens abgegeben.
    pub(crate) fn read_line_threads(&mut self) -> R<Option<String>> {
        if !self.w.is_active() || !matches!(self.w.st().console, console::Console::Stdio) {
            return Ok(self.w.st().console.read_line());
        }
        self.w.st().external.insert(self.tid);
        self.w.bump();
        self.w.release();
        let line = console::Console::Stdio.read_line();
        self.w.acquire();
        self.w.st().external.remove(&self.tid);
        if self.w.cancelled() && self.fatals.get() == 0 {
            return Err(self.cancelled());
        }
        Ok(line)
    }

    fn sync_object(&mut self, class: &str, id: i64) -> R<Value> {
        let Some(d) = self.p.std_def(class) else { return Err(self.internal(format!("{class} fehlt"))) };
        Ok(Value::Obj(self.raw_object(d, vec![Value::I64(id)]), Form::Own))
    }

    /// Thread- oder Task-Handle freigeben (R-16.4).
    fn release_handle(&mut self, id: i64) -> R<Value> {
        let Some(t) = self.w.st().threads.get(&id) else { return Ok(Value::Void) };
        if !t.joined {
            if id == self.tid {
                self.w.st().threads.get_mut(&id).unwrap().joined = true;
                return Err(self.thread_error("A thread cannot wait for itself"));
            }
            let unwinding = self.fatals.get() > 0;
            self.wait_until(|st| Self::thread_done(st, id, unwinding))?;
            let result = self.w.st().threads.get_mut(&id).and_then(|t| t.result.take());
            match result {
                Some(Ok(v)) => self.drop_value(v)?,
                Some(Err((ex, _))) => {
                    let failed = self.fatals.get() == 0;
                    let name = ex.obj().and_then(|o| o.class).map(|d| self.p.defs[d].name.clone()).unwrap_or_default();
                    let msg = if failed { self.describe(&ex) } else { String::new() };
                    let _ = self.drop_value(ex);
                    if failed {
                        self.w.st().threads.remove(&id);
                        return Err(self.thread_error(format!("A thread ended with an exception that nobody retrieved with join: {name}: {msg}")));
                    }
                }
                None => {}
            }
        }
        let st = self.w.st();
        if st.threads.get(&id).is_some_and(|t| t.done) {
            st.threads.remove(&id);
        }
        Ok(Value::Void)
    }

    fn join(&mut self, id: i64) -> R<Value> {
        match self.w.st().threads.get(&id) {
            Some(t) if !t.joined => {}
            _ => return Err(self.thread_error("The thread has already been joined")),
        }
        if id == self.tid {
            return Err(self.thread_error("A thread cannot join itself"));
        }
        self.wait_until(|st| Self::thread_done(st, id, false))?;
        let Some(t) = self.w.st().threads.get_mut(&id) else { return Ok(Value::Void) };
        t.joined = true;
        match t.result.take() {
            Some(Ok(v)) => Ok(v),
            Some(Err((ex, trace))) => Err(Flow::Throw(ex, trace)),
            None => Err(self.cancelled()),
        }
    }

    /// `Thread::scope` (R-16.11, R-16.13)
    fn scope(&mut self, body: Rc<Closure>) -> R<Value> {
        let st = self.w.st();
        let sid = st.next_id;
        st.next_id += 1;
        st.scopes.insert(sid, Vec::new());
        let scope = self.sync_object("rpp.sync.Scope", sid)?;
        let r = self.call_closure(&body, &mut [scope.view()]);
        let ids = self.w.st().scopes.remove(&sid).unwrap_or_default();
        // Alle Threads des Bereichs enden vor seinem Ende; ein Fehler beim Warten (Abbruch,
        // Verklemmung) wickelt diesen Thread ab, gewartet wird trotzdem.
        let mut failed = None;
        for id in &ids {
            let unwinding = self.fatals.get() > 0;
            if let Err(e) = self.wait_until(|st| Self::thread_done(st, *id, unwinding)) {
                failed.get_or_insert(e);
                let _ = self.wait_until(|st| Self::thread_done(st, *id, true));
            }
        }
        let mut first: Option<(Value, Vec<String>)> = None;
        for id in &ids {
            let done = self.w.st().threads.get(&id).is_some_and(|t| t.done);
            let result = self.w.st().threads.get_mut(&id).and_then(|t| t.result.take());
            if done {
                self.w.st().threads.remove(id);
            }
            match result {
                Some(Err(ex)) if first.is_none() && r.is_ok() && failed.is_none() => first = Some(ex),
                Some(Err((ex, _))) => {
                    let _ = self.drop_value(ex);
                }
                Some(Ok(v)) => {
                    let _ = self.drop_value(v);
                }
                None => {}
            }
        }
        let d = self.drop_value(scope);
        match (r, failed, first) {
            (Err(Flow::Fatal(f)), _, _) => Err(Flow::Fatal(f)),
            (r, Some(e), _) => {
                if let Err(Flow::Throw(ex, _)) = r {
                    let _ = self.drop_value(ex);
                }
                Err(e)
            }
            (Err(e), None, _) => Err(e),
            (Ok(_), None, Some((ex, trace))) => Err(Flow::Throw(ex, trace)),
            (Ok(_), None, None) => d.map(|_| Value::Void),
        }
    }

    fn sleep(&mut self, millis: i64) -> R<Value> {
        let total = Duration::from_millis(millis.max(0) as u64);
        if !self.w.is_active() {
            std::thread::sleep(total);
            return Ok(Value::Void);
        }
        let start = Instant::now();
        loop {
            if self.w.cancelled() && self.fatals.get() == 0 {
                return Err(self.cancelled());
            }
            let Some(rest) = total.checked_sub(start.elapsed()) else { return Ok(Value::Void) };
            let seen = self.w.generation();
            self.w.wait_change(seen, Some(rest));
        }
    }

    fn channel(&mut self, id: i64) -> Option<&'static mut Chan> {
        self.w.st().channels.get_mut(&id)
    }

    fn send(&mut self, id: i64, value: Value) -> R<Value> {
        let mut value = Some(value);
        loop {
            let Some(c) = self.channel(id) else { break };
            if !c.receiver {
                break;
            }
            if c.queue.len() < c.cap {
                c.queue.push_back(value.take().unwrap());
                self.w.bump();
                return Ok(Value::Bool(true));
            }
            if let Err(e) = self.wait_until(|st| st.channels.get(&id).is_none_or(|c| !c.receiver || c.queue.len() < c.cap)) {
                let _ = self.drop_value(value.take().unwrap());
                return Err(e);
            }
        }
        // R-16.15: Der Empfänger ist freigegeben.
        self.drop_value(value.take().unwrap())?;
        Ok(Value::Bool(false))
    }

    fn receive(&mut self, id: i64) -> R<Value> {
        loop {
            let Some(c) = self.channel(id) else { return Ok(Value::Null) };
            if let Some(v) = c.queue.pop_front() {
                self.w.bump();
                return Ok(v);
            }
            if c.senders == 0 {
                return Ok(Value::Null);
            }
            self.wait_until(|st| st.channels.get(&id).is_none_or(|c| !c.queue.is_empty() || c.senders == 0))?;
        }
    }

    pub(crate) fn native_sync(&mut self, name: &str, m: &MethodInfo, o: Option<ORef>, args: &mut [Value]) -> R<Value> {
        let id = o.as_ref().map(obj_id).unwrap_or(0);
        self.w.st().sync_used = true;
        match name {
            "Thread.spawn" | "Thread.spawnWith" => {
                let with = name == "Thread.spawnWith";
                let input = with.then(|| take_argument(&mut args[0]));
                let Value::Func(body) = take_argument(&mut args[with as usize]) else { return Err(self.internal("Thread ohne Rumpf")) };
                let id = self.start_thread(body, input)?;
                let class = if class_of(&m.ret.ty()).is_some_and(|d| self.p.defs[d].name == "Task") { "rpp.sync.Task" } else { "rpp.sync.Thread" };
                self.sync_object(class, id)
            }
            "Thread.scope" => {
                let Some(Value::Func(body)) = args.first().cloned() else { return Err(self.internal("Bereich ohne Rumpf")) };
                self.scope(body)
            }
            "Thread.sleep" => self.sleep(args.first().and_then(Value::as_i128).unwrap_or(0) as i64),
            "Thread.join" | "Task.join" => self.join(id),
            "Thread.release" | "Task.release" => self.release_handle(id),
            "Scope.spawn" => {
                let Some(Value::Func(body)) = args.first().cloned() else { return Err(self.internal("Thread ohne Rumpf")) };
                let tid = self.start_thread(body, None)?;
                if let Some(list) = self.w.st().scopes.get_mut(&id) {
                    list.push(tid);
                }
                Ok(Value::Void)
            }
            "Channel.open" => {
                let cap = args.first().and_then(Value::as_i128).unwrap_or(0);
                if cap < 1 {
                    return Err(self.thread_error(format!("The capacity of a channel must be at least 1, got {cap}")));
                }
                let st = self.w.st();
                let id = st.next_id;
                st.next_id += 1;
                st.channels.insert(id, Chan { queue: VecDeque::new(), cap: cap as usize, senders: 1, receiver: true });
                let tx = self.sync_object("rpp.sync.Sender", id)?;
                let rx = self.sync_object("rpp.sync.Receiver", id)?;
                Ok(Value::Tuple(vec![tx, rx]))
            }
            "Sender.send" => self.send(id, take_argument(&mut args[0])),
            "Sender.clone" => {
                if let Some(c) = self.channel(id) {
                    c.senders += 1;
                }
                self.sync_object("rpp.sync.Sender", id)
            }
            "Sender.close" => {
                if let Some(c) = self.channel(id) {
                    c.senders = c.senders.saturating_sub(1);
                    if c.senders == 0 && !c.receiver {
                        self.w.st().channels.remove(&id);
                    }
                    self.w.bump();
                }
                Ok(Value::Void)
            }
            "Receiver.receive" => self.receive(id),
            "Receiver.close" => {
                let mut left = VecDeque::new();
                if let Some(c) = self.channel(id) {
                    c.receiver = false;
                    left = std::mem::take(&mut c.queue);
                    if c.senders == 0 {
                        self.w.st().channels.remove(&id);
                    }
                    self.w.bump();
                }
                // Werte, die niemand mehr empfängt, werden freigegeben.
                let mut r = Ok(());
                for v in left {
                    r = Self::combine(r, self.drop_value(v));
                }
                r.map(|_| Value::Void)
            }
            _ => Err(self.internal(format!("rpp.sync.{name}"))),
        }
    }
}
