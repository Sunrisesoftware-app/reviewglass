//! Session source (rg.session-source): the boundary where the two Claude Code surfaces
//! are reconciled into one list of sessions.
//!
//! v1 ships both implementations, not one plus a fallback (adr.rg.003). statusLine runs
//! in the terminal CLI only, so `ClaudeStatusLineSource` reaches CLI sessions through
//! the spool, and `ClaudeTranscriptSource` reads the JSONL under `~/.claude/projects/`,
//! which is the only channel that reaches a Desktop session at all.

pub mod payload;
