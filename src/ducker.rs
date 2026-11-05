use crate::audio::{Audio, Session};
use crate::config::{self, Config};
use std::collections::HashMap;
use std::time::Instant;

/// Difference treated as "someone else changed the volume".
/// The mixer moves in 1% steps, so this must stay below 0.01.
const USER_CHANGE_EPS: f32 = 0.004;

#[derive(Clone, Copy)]
struct Fade {
    from: f32,
    to: f32,
    start: Instant,
}

#[derive(Default)]
struct Prog {
    /// Volume to duck to. Starts from config, replaced if the user changes it while ducked.
    unfocused: f32,
    /// Volume to restore on focus. `Some` = ducked. Kept even while the program isn't
    /// running, because Windows remembers the ducked volume for the next launch.
    saved: Option<f32>,
    /// Last volume we set, per session instance id. A session whose current volume
    /// differs was changed by someone else (the user). New sessions aren't in here,
    /// so they are never mistaken for a user change.
    last_set: HashMap<String, f32>,
    fade: Option<Fade>,
}

impl Prog {
    /// A volume the user set on one of our tracked sessions, if any.
    fn user_change(&self, sess: &[&Session]) -> Option<f32> {
        sess.iter().find_map(|s| {
            let l = self.last_set.get(&s.id)?;
            let v = s.get();
            ((v - l).abs() > USER_CHANGE_EPS).then_some(v)
        })
    }

    /// Current level: a tracked session if possible (all tracked ones are equal), else any.
    fn level(&self, sess: &[&Session]) -> f32 {
        sess.iter()
            .find(|s| self.last_set.contains_key(&s.id))
            .or(sess.first())
            .map_or(1.0, |s| s.get())
    }

    fn set(&mut self, sess: &[&Session], v: f32) {
        for s in sess {
            s.set(v);
            self.last_set.insert(s.id.clone(), v);
        }
    }

    fn start_fade(&mut self, sess: &[&Session], from: f32, to: f32) {
        self.fade = Some(Fade { from, to, start: Instant::now() });
        // Snapshot current values so the fade doesn't mistake old state for a user change.
        self.last_set = sess.iter().map(|s| (s.id.clone(), s.get())).collect();
    }
}

pub struct Ducker {
    audio: Audio,
    cfg: Config,
    progs: HashMap<String, Prog>,
    sessions: Vec<Session>,
    foreground: String,
    pub paused: bool,
    /// Restore levels loaded from restore.txt, applied to programs on config load.
    pending: HashMap<String, f32>,
    restore_written: String,
}

fn of<'a>(sessions: &'a [Session], exe: &str) -> Vec<&'a Session> {
    sessions.iter().filter(|s| s.exe == exe).collect()
}

fn restore_path() -> std::path::PathBuf {
    config::dir().join("restore.txt")
}

impl Ducker {
    pub fn new(audio: Audio, cfg: Config) -> Self {
        // Volumes still ducked from the previous run (crash, or the game wasn't running at exit).
        let pending = std::fs::read_to_string(restore_path())
            .unwrap_or_default()
            .lines()
            .filter_map(|l| {
                let (exe, v) = l.split_once('\t')?;
                Some((exe.to_string(), v.parse().ok()?))
            })
            .collect();
        let mut d = Ducker {
            audio,
            cfg: Config::default(),
            progs: HashMap::new(),
            sessions: Vec::new(),
            foreground: String::new(),
            paused: false,
            pending,
            restore_written: String::new(),
        };
        d.apply_config(cfg);
        d
    }

    fn write_restore_file(&mut self) {
        let mut text: String = self
            .progs
            .iter()
            .filter_map(|(exe, p)| p.saved.map(|v| format!("{exe}\t{v}\n")))
            .collect();
        text.extend(self.pending.iter().map(|(exe, v)| format!("{exe}\t{v}\n")));
        if text == self.restore_written {
            return;
        }
        if text.is_empty() {
            let _ = std::fs::remove_file(restore_path());
        } else {
            let _ = std::fs::write(restore_path(), &text);
        }
        self.restore_written = text;
    }

    /// Returns true if a fade timer is needed.
    pub fn apply_config(&mut self, cfg: Config) -> bool {
        let mut new = HashMap::new();
        for p in &cfg.programs {
            let exe = p.exe.trim().to_lowercase();
            let mut prog = self.progs.remove(&exe).unwrap_or_default();
            prog.unfocused = p.unfocused.unwrap_or(cfg.default_unfocused).clamp(0.0, 1.0);
            if let Some(v) = self.pending.remove(&exe) {
                prog.saved.get_or_insert(v);
            }
            new.insert(exe, prog);
        }
        // Programs removed from the config: give their volume back.
        for (exe, p) in self.progs.drain() {
            if let Some(v) = p.saved {
                of(&self.sessions, &exe).iter().for_each(|s| s.set(v));
            }
        }
        self.progs = new;
        self.cfg = cfg;
        self.refresh()
    }

    /// Returns true if a fade timer is needed.
    pub fn on_focus(&mut self, exe: String) -> bool {
        self.foreground = exe;
        self.refresh()
    }

    /// Returns true if a fade timer is needed.
    pub fn set_paused(&mut self, paused: bool) -> bool {
        self.paused = paused;
        if paused {
            self.restore_all();
            false
        } else {
            self.refresh()
        }
    }

    /// Instantly restore everything that is ducked and running (pause / exit).
    /// Programs that aren't running keep their saved level in restore.txt.
    pub fn restore_all(&mut self) {
        self.sessions = self.audio.sessions(|e| self.progs.contains_key(e));
        for (exe, p) in self.progs.iter_mut() {
            p.fade = None;
            let sess = of(&self.sessions, exe);
            if sess.is_empty() {
                continue;
            }
            if let Some(v) = p.saved.take() {
                sess.iter().for_each(|s| s.set(v));
            }
            p.last_set.clear();
        }
        self.write_restore_file();
    }

    fn refresh(&mut self) -> bool {
        if self.paused {
            return false;
        }
        // Re-enumerate on every focus change: picks up new sessions and device switches.
        self.sessions = self.audio.sessions(|e| self.progs.contains_key(e));

        for (exe, p) in self.progs.iter_mut() {
            let sess = of(&self.sessions, exe);
            p.last_set.retain(|id, _| sess.iter().any(|s| &s.id == id));
            if sess.is_empty() {
                p.fade = None;
                continue;
            }
            // User changed the volume while ducked (e.g. in the mixer): that's the new ducked level.
            if p.saved.is_some() {
                if let Some(v) = p.user_change(&sess) {
                    p.unfocused = v;
                    p.fade = None;
                    p.set(&sess, v); // also brings other/new sessions of this program to it
                }
            }
            let focused = *exe == self.foreground;
            match (focused, p.saved) {
                (true, Some(saved)) => {
                    p.saved = None;
                    let from = p.level(&sess);
                    p.start_fade(&sess, from, saved);
                }
                (false, None) => {
                    let cur = p.level(&sess);
                    // If we were mid-restore, the real "focused" level is the fade target.
                    p.saved = Some(p.fade.map_or(cur, |f| f.to));
                    p.start_fade(&sess, cur, p.unfocused);
                }
                (false, Some(_)) if p.fade.is_none() => {
                    // Still ducked: duck sessions that appeared since (they start at full volume).
                    let new: Vec<&Session> =
                        sess.iter().copied().filter(|s| !p.last_set.contains_key(&s.id)).collect();
                    let v = p.unfocused;
                    p.set(&new, v);
                }
                _ => {}
            }
        }
        self.write_restore_file();
        self.tick()
    }

    /// Advance fades. Returns true while any fade is still running.
    pub fn tick(&mut self) -> bool {
        let now = Instant::now();
        let dur = self.cfg.fade_ms as f32 / 1000.0;
        let mut active = false;
        for (exe, p) in self.progs.iter_mut() {
            let Some(f) = p.fade else { continue };
            let sess = of(&self.sessions, exe);
            // User grabbed the slider mid-fade: stop fighting. Picked up on the next focus change.
            if p.user_change(&sess).is_some() {
                p.fade = None;
                continue;
            }
            let t = if dur <= 0.0 { 1.0 } else { ((now - f.start).as_secs_f32() / dur).min(1.0) };
            let eased = 1.0 - (1.0 - t) * (1.0 - t);
            p.set(&sess, f.from + (f.to - f.from) * eased);
            if t >= 1.0 {
                p.fade = None;
            } else {
                active = true;
            }
        }
        active
    }
}
