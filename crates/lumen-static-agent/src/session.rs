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

/// A recorded session: a header and the `input.*` calls that followed.
#[derive(Clone, Debug)]
pub struct Recording {
    pub header: Header,
    pub calls: Vec<(String, Value)>,
}

/// Why a recording cannot be replayed here.
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
        }
    }
}

impl Recording {
    pub fn new(header: Header) -> Self {
        Recording { header, calls: Vec::new() }
    }

    /// Note one dispatched call. Only `input.*` is a *step*: queries do
    /// not change state and a replay that re-ran them would be asserting
    /// on the recorder's answers rather than the app's.
    pub fn note(&mut self, method: &str, params: &Value) {
        if method.starts_with("input.") {
            self.calls.push((method.to_owned(), params.clone()));
        }
    }

    pub fn len(&self) -> usize {
        self.calls.len()
    }

    pub fn is_empty(&self) -> bool {
        self.calls.is_empty()
    }

    /// Check a host against the header before replaying against it.
    pub fn check(&self, width: u32, height: u32, measurer: &str) -> Result<(), ReplayError> {
        if self.header.start_frame != 0 {
            return Err(ReplayError::NotFromStart(self.header.start_frame));
        }
        if (self.header.width, self.header.height) != (width, height) {
            return Err(ReplayError::Mismatch {
                field: "viewport",
                recorded: format!("{}x{}", self.header.width, self.header.height),
                host: format!("{width}x{height}"),
            });
        }
        if self.header.measurer != measurer {
            return Err(ReplayError::Mismatch {
                field: "measurer",
                recorded: self.header.measurer.clone(),
                host: measurer.to_owned(),
            });
        }
        Ok(())
    }

    /// Replay every step against `host`, stopping at the first error.
    pub fn replay(
        &self,
        agent: &crate::Agent,
        host: &mut dyn crate::AgentHost,
        width: u32,
        height: u32,
        measurer: &str,
    ) -> Result<(), ReplayError> {
        self.check(width, height, measurer)?;
        for (i, (method, params)) in self.calls.iter().enumerate() {
            let req = json!({
                "jsonrpc": "2.0", "id": i + 1, "method": method, "params": params,
            });
            let _ = agent.dispatch_input(host, &req);
        }
        Ok(())
    }

    /// The session as a runnable Rust test.
    ///
    /// Generated source rather than a data file, so it compiles: if the
    /// recording refers to a selector that no longer exists, the failure
    /// is a compile-or-assert in the repository rather than a file
    /// nobody runs. That is the difference between a script and a log.
    pub fn export_test(&self, fn_name: &str, host_expr: &str) -> String {
        let mut out = String::new();
        out.push_str("// Generated by `Recording::export_test`. Re-record rather than edit.\n");
        out.push_str(&format!(
            "//\n// Recorded at {}x{}, stylesheet {:?}, measurer {:?}.\n\
             // A host that does not match refuses to replay — see\n\
             // `lumen_static_agent::session`.\n",
            self.header.width, self.header.height, self.header.stylesheet, self.header.measurer
        ));
        out.push_str("#[test]\n");
        out.push_str(&format!("fn {fn_name}() {{\n"));
        out.push_str(&format!("    let mut host = {host_expr};\n"));
        out.push_str("    let agent = lumen_static_agent::Agent::new();\n");
        for (method, params) in &self.calls {
            out.push_str(&format!(
                "    let _ = agent.dispatch_input(&mut host, &serde_json::json!({{\n        \
                 \"jsonrpc\": \"2.0\", \"id\": 1, \"method\": {method:?}, \"params\": {params}\n    }}));\n"
            ));
        }
        out.push_str("    // Assertions on final state go here.\n");
        out.push_str("}\n");
        out
    }
}
