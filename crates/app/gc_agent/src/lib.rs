//! The AI agent API (PLAN 9.8). An MCP server on `127.0.0.1` lets an agent such as Claude Code or Claude
//! Desktop read and edit the open robot cell with tools. The server's threads never touch the document:
//! each call is queued through [`Bridge`] and carried out by [`execute`] on the application's thread, so
//! agent edits are ordinary commands that show at once and undo like the user's own.

mod exec;
mod server;

pub use exec::{Call, execute};
pub use server::{Bridge, GraphCtrlTools, Request, Server, new_token, start};
