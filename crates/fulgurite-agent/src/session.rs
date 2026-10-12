//! Recording an agent session so it replays as a test — plan M12/T12.3.
//!
//! The invariant has held since E4: every synthesized input enters the
//! host's one queue, so a session *is* a script. What was missing is
//! that **nothing wrote the recording out**, and a claim nothing
//! exercises is the shape of claim this crate's own audit exists to
//! refuse. (It was exercised for a single click at the `Event` level —
//! `tests/input.rs` — which is narrower than the claim.)
//!
//! # Which level records, and why
//!
//! **The RPC level** — `("input.click", {"selector": "#inc"})` — not the
//! `Event` level.
//!
//! A selector survives layout drift: restyle the button, move it, and
//! `#inc` still names it. A recorded *coordinate* freezes the geometry
//! of the machine that recorded it, and the first thing that changes a
//! layout invalidates every step. The trade is the mirror image — a
//! selector breaks when an author *renames* the node — and that is the
//! better failure, because it is loud and it is a real change to the
//! app rather than an incidental one.
//!
//! # A list of calls is not enough to replay
//!
//! Four things decide where a replayed click lands and none of them is
//! an `input.*` call. They go in a [`Header`], and a replay against a
//! host that does not match it is a **hard error rather than a diff**:
//!
//! - **viewport** — a selector resolves to the node's laid-out centre,
//!   so a different size moves it;
//! - **stylesheet** — padding, radius and `font_size` reach `NodeState`
//!   and therefore the geometry, and a stylesheet is not an input;
//! - **start frame** — `scripts/live_gate.sh` warms a window with twenty
//!   clicks before any check, so a recording from a live session does
//!   not begin at `Default`;
//! - **measurer** — label width decides node size decides node centre.
//!   This repository has already been burned by exactly that mismatch:
//!   the audit measured with the block stand-in while the window laid
//!   out with a real face (`docs/results-frame-cost.md`).
//!
//! Timing is deliberately absent: there is no frame clock. The residue
//! is that an exported test must not assert on frame numbers, `app.perf`
//! figures, or `input.scrollIntoView`'s step count.
//!
//! # A replay that diverges is an error (M48/T48.10)
//!
//! Each step keeps a summary of its reply — `handled`, and the error code
//! if it failed — and [`Recording::replay`] compares: a step that errors,
//! or whose `handled` differs from what was recorded, is
//! [`ReplayError::Diverged`], naming the step. The stylesheet is checked
//! with the rest of the header. [`Recording::export_test`] asserts every
//! step's reply and ends on the recorded `ui.getTree`'s [`digest`].
//!
//! Until then a replay discarded every reply (`let _ =`), `check` took no
//! stylesheet although this doc listed it, and an export emitted `let _ =`
//! per step and a placeholder comment where its assertion belonged — so a
//! corrupted recording replayed `Ok`, a mismatched sheet replayed `Ok`,
//! and an export clicking a selector that no longer existed compiled and
//! passed (R13-F6, `tests/session_replay_fails.rs`).

use serde_json::{json, Value};

/// What a replay host must match for a recording to mean anything.
#[derive(Clone, Debug, PartialEq)]
pub struct Header {
    pub width: u32,
    pub height: u32,
    /// An author-chosen name for the stylesheet in force. Not the rules
    /// themselves: they are code, and a recording that carried them
    /// would be asserting on a copy rather than on the app.
    pub stylesheet: String,
    /// Which measurer laid the session out — `"block"` for the
    /// deterministic stand-in, a face name for a real stack.
    pub measurer: String,
    /// The frame the recording began on. Non-zero means the host was
    /// already driven, and a fresh replay cannot reproduce it.
    pub start_frame: u64,
}

impl Header {
    pub fn to_json(&self) -> Value {
        json!({
            "width": self.width,
            "height": self.height,
            "stylesheet": self.stylesheet,
            "measurer": self.measurer,
            "start_frame": self.start_frame,
        })
    }
}

/// What a step's reply said — M48/T48.10: enough to know whether a
/// replay did the same thing, and nothing an app's timing changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Outcome {
    /// The reply's `handled`, when the method reports one.
    pub handled: Option<bool>,
    /// The error code, when the step failed.
    pub error: Option<i64>,
}

impl Outcome {
    /// Summarise a JSON-RPC reply.
    pub fn of(reply: &Value) -> Self {
        Outcome {
            handled: reply.get("result").and_then(|r| r.get("handled")).and_then(Value::as_bool),
            error: reply.get("error").and_then(|e| e.get("code")).and_then(Value::as_i64),
        }
    }
}

impl core::fmt::Display for Outcome {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match (self.error, self.handled) {
            (Some(c), _) => write!(f, "error {c}"),
            (None, Some(h)) => write!(f, "handled {h}"),
            (None, None) => write!(f, "ok"),
        }
    }
}

/// A recorded session: a header and the `input.*` calls that followed,
/// each with what its reply said.
#[derive(Clone, Debug)]
pub struct Recording {
    pub header: Header,
    pub calls: Vec<(String, Value)>,
    /// One per call: what the recorded reply said (M48/T48.10).
    pub outcomes: Vec<Outcome>,
}

/// Why a recording cannot be replayed here, or did not replay.
#[derive(Debug, PartialEq)]
pub enum ReplayError {
    /// The host does not match the header. Never a diff — a session
    /// replayed against different geometry produces clicks that land
    /// somewhere else, and a *passing* run would be worse than a
    /// failing one.
    Mismatch { field: &'static str, recorded: String, host: String },
    /// The recording began mid-session and a fresh host cannot
    /// reproduce its starting state.
    NotFromStart(u64),
    /// Step `step` (1-based) did not do what it did when recorded: it
    /// failed, or its `handled` differs — M48/T48.10.
    Diverged { step: usize, method: String, recorded: Outcome, got: Outcome },
}

impl core::fmt::Display for ReplayError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ReplayError::Mismatch { field, recorded, host } => write!(
                f,
                "recording was made with {field} = {recorded}, replay host has {host}: a click \
                 would land somewhere else, so this is refused rather than diffed"
            ),
            ReplayError::NotFromStart(n) => write!(
                f,
                "recording starts at frame {n}, not 0 — a fresh host cannot reproduce the state \
                 it began from; record from construction"
            ),
            ReplayError::Diverged { step, method, recorded, got } => write!(
                f,
                "step {step} (`{method}`) diverged: recorded {recorded}, replay {got}"
            ),
        }
    }
}

/// A stable 64-bit digest of a JSON value (FNV-1a over its text) — what
/// an exported test compares its final `ui.getTree` against.
pub fn digest(v: &Value) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in v.to_string().bytes() {
        h = (h ^ b as u64).wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

impl Header {
    /// Check a host against this header — M48/T48.10 added the
    /// stylesheet, which the module doc had always listed.
    pub fn check(&self, width: u32, height: u32, stylesheet: &str, measurer: &str) -> Result<(), ReplayError> {
        if self.start_frame != 0 {
            return Err(ReplayError::NotFromStart(self.start_frame));
        }
        if (self.width, self.height) != (width, height) {
            return Err(ReplayError::Mismatch {
                field: "viewport",
                recorded: format!("{}x{}", self.width, self.height),
                host: format!("{width}x{height}"),
            });
        }
        if self.stylesheet != stylesheet {
            return Err(ReplayError::Mismatch {
                field: "stylesheet",
                recorded: self.stylesheet.clone(),
                host: stylesheet.to_owned(),
            });
        }
        if self.measurer != measurer {
            return Err(ReplayError::Mismatch {
                field: "measurer",
                recorded: self.measurer.clone(),
                host: measurer.to_owned(),
            });
        }
        Ok(())
    }
}

impl Recording {
    pub fn new(header: Header) -> Self {
        Recording { header, calls: Vec::new(), outcomes: Vec::new() }
    }

    /// Note one dispatched call and its reply. Only `input.*` is a
    /// *step*: queries do not change state and a replay that re-ran them
    /// would be asserting on the recorder's answers rather than the app's.
    pub fn note(&mut self, method: &str, params: &Value, reply: &Value) {
        if method.starts_with("input.") {
            self.calls.push((method.to_owned(), params.clone()));
            self.outcomes.push(Outcome::of(reply));
        }
    }

    pub fn len(&self) -> usize {
        self.calls.len()
    }

    pub fn is_empty(&self) -> bool {
        self.calls.is_empty()
    }

    /// Check a host against the header before replaying against it.
    pub fn check(&self, width: u32, height: u32, stylesheet: &str, measurer: &str) -> Result<(), ReplayError> {
        self.header.check(width, height, stylesheet, measurer)
    }

    /// Replay every step against `host`, stopping at the first that
    /// diverges — errors, or answers a different `handled` than it was
    /// recorded with.
    pub fn replay(
        &self,
        agent: &crate::Agent,
        host: &mut dyn crate::AgentHost,
        width: u32,
        height: u32,
        stylesheet: &str,
        measurer: &str,
    ) -> Result<(), ReplayError> {
        self.check(width, height, stylesheet, measurer)?;
        for (i, (method, params)) in self.calls.iter().enumerate() {
            let req = json!({
                "jsonrpc": "2.0", "id": i + 1, "method": method, "params": params,
            });
            let got = Outcome::of(&agent.dispatch_input(host, &req));
            let recorded = self.outcomes.get(i).copied().unwrap_or_default();
            if got.error.is_some() || got.handled != recorded.handled {
                return Err(ReplayError::Diverged { step: i + 1, method: method.clone(), recorded, got });
            }
        }
        Ok(())
    }

    /// The session as a runnable Rust test: [`Recording::export_fn`]
    /// under `#[test]`.
    ///
    /// Generated source rather than a data file, so it compiles: if the
    /// recording refers to a selector that no longer exists, the failure
    /// is a compile-or-assert in the repository rather than a file
    /// nobody runs. That is the difference between a script and a log.
    ///
    /// `final_tree` is `ui.getTree`'s `result` at the end of the recorded
    /// session; the test ends by comparing its own against its
    /// [`digest`]. `host_expr` must evaluate to a host implementing
    /// `AgentHost`; `host_props` to `(width, height, stylesheet,
    /// measurer)` for that host, which the header is checked against.
    pub fn export_test(&self, fn_name: &str, host_expr: &str, host_props: &str, final_tree: &Value) -> String {
        format!("#[test]\n{}", self.export_fn(fn_name, host_expr, host_props, final_tree))
    }

    /// The body of [`Recording::export_test`] as a plain `fn` — so a test
    /// can include an export and run it where it must panic.
    pub fn export_fn(&self, fn_name: &str, host_expr: &str, host_props: &str, final_tree: &Value) -> String {
        let h = &self.header;
        let mut out = String::new();
        out.push_str("// Generated by `Recording::export_test`. Re-record rather than edit.\n");
        out.push_str(&format!(
            "//\n// Recorded at {}x{}, stylesheet {:?}, measurer {:?}.\n\
             // A host that does not match refuses to replay — see\n\
             // `fulgurite_agent::session`.\n",
            h.width, h.height, h.stylesheet, h.measurer
        ));
        out.push_str(&format!("pub fn {fn_name}() {{\n"));
        out.push_str(&format!(
            "    let header = fulgurite_agent::session::Header {{ width: {}, height: {}, \
             stylesheet: {:?}.into(), measurer: {:?}.into(), start_frame: {} }};\n",
            h.width, h.height, h.stylesheet, h.measurer, h.start_frame
        ));
        out.push_str(&format!("    let (w, hgt, sheet, measurer) = {host_props};\n"));
        out.push_str("    header.check(w, hgt, sheet, measurer).expect(\"the host does not match the recording\");\n");
        out.push_str(&format!("    let mut host = {host_expr};\n"));
        out.push_str("    let agent = fulgurite_agent::Agent::new();\n");
        for (k, (method, params)) in self.calls.iter().enumerate() {
            let step = k + 1;
            let o = self.outcomes.get(k).copied().unwrap_or_default();
            out.push_str(&format!(
                "    let r = agent.dispatch_input(&mut host, &serde_json::json!({{\n        \
                 \"jsonrpc\": \"2.0\", \"id\": {step}, \"method\": {method:?}, \"params\": {params}\n    }}));\n"
            ));
            out.push_str(&format!(
                "    assert!(r.get(\"error\").is_none(), \"step {step} (`{method}`) failed: {{r}}\");\n"
            ));
            if let Some(hd) = o.handled {
                out.push_str(&format!(
                    "    assert_eq!(r[\"result\"][\"handled\"], {hd}, \"step {step} (`{method}`): handled\");\n"
                ));
            }
        }
        out.push_str(
            "    let tree = agent.dispatch_input(&mut host, &serde_json::json!({\"jsonrpc\": \"2.0\", \
             \"id\": 0, \"method\": \"ui.getTree\"}));\n",
        );
        out.push_str(&format!(
            "    assert_eq!(fulgurite_agent::session::digest(&tree[\"result\"]), {:#x}, \
             \"the final tree differs from the recorded one\");\n",
            digest(final_tree)
        ));
        out.push_str("}\n");
        out
    }
}
